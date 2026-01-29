#[macro_use]
extern crate rocket;

use rocket::fs::{FileServer, relative};
use rocket::figment::{Figment, providers::{Env, Toml, Format}};
use rocket::fairing::AdHoc;
use rocket_dyn_templates::Template;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

mod schema;
mod db;
mod models;
mod auth;
mod state;
mod settings;
mod routes;
mod action_ttl;
mod pending_cleanup;

use db::{initialize, get_conn};
use state::{SystemState, AppState};

#[launch]
fn rocket() -> _ {
    // logging ONCE
    flexi_logger::Logger::try_with_env_or_str(
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
    )
    .unwrap()
    .log_to_stdout()
    .start()
    .unwrap();

    let figment = Figment::from(rocket::Config::default())
        .merge(Toml::file("Rocket.toml").nested())
        .merge(Env::prefixed("ROCKET_").global());

    let db_pool = initialize();

    let settings = {
        let mut conn = get_conn(&db_pool);
        Arc::new(RwLock::new(settings::ServerSettings::load(&mut conn)))
    };

    let system = SystemState::new(db_pool.clone());

    // IMPORTANT: AppState is NOT wrapped in Arc here
    let app_state = AppState {
        db_pool: db_pool.clone(),
        system: system.clone(),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings,
        log_audit: Some(Arc::new(move |conn, actor, action, target, details| {
            if let Err(e) = db::log_audit(conn, actor, action, target, details) {
                log::error!("Audit logging failed: {:?}", e);
            }
            Ok(())
        })),
    };

    app_state.system.refresh();
    log::info!(
        "System memory: total {} MB, available {} MB",
        app_state.system.total_memory() / 1024 / 1024,
        app_state.system.available_memory() / 1024 / 1024
    );

    rocket::custom(figment)
        .manage(db_pool)
        .manage(app_state) // <-- FIXED

        .attach(Template::fairing())
        .attach(action_ttl::ActionTtlFairing)
        .attach(pending_cleanup::PendingCleanupFairing)

        .attach(AdHoc::on_liftoff("Startup Audit", |rocket| {
            let app = rocket.state::<AppState>();
            Box::pin(async move {
                if let Some(app) = app {
                    let mut conn = get_conn(&app.db_pool);
                    let _ = (app.log_audit.as_ref().unwrap())(
                        &mut conn,
                        "system",
                        "server_started",
                        None,
                        Some("PatchPilot server started"),
                    );
                }
            })
        }))

        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::routes())
        .mount("/roles", routes::roles::routes())
        .mount("/settings", routes::settings::routes())
        .mount("/static", FileServer::from(relative!("static")))
        .mount("/", routes::page_routes())
}
