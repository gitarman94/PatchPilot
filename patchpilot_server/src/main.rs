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

use db::{DbPool, initialize, get_conn};
use state::{SystemState, AppState};

#[launch]
fn rocket() -> _ {
    // Initialize logging
    flexi_logger::Logger::try_with_env_or_str(
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
    )
    .unwrap()
    .log_to_stdout()
    .start()
    .unwrap();

    // Build Rocket configuration from Rocket.toml and ROCKET_* env vars
    let figment = Figment::from(rocket::Config::default())
        .merge(Toml::file("Rocket.toml").nested())
        .merge(Env::prefixed("ROCKET_").global());

    rocket::custom(figment)
        // Create and manage the database connection pool
        .manage(initialize())

        // Initialize application state after Rocket ignition
        .attach(AdHoc::on_ignite("Init AppState", |rocket| async move {
            let pool = rocket.state::<DbPool>().unwrap().clone();

            // Load persistent server settings from the database
            let settings = {
                let mut conn = get_conn(&pool);
                Arc::new(RwLock::new(settings::ServerSettings::load(&mut conn)))
            };

            // Initialize system metrics and runtime state
            let system = SystemState::new(pool.clone());

            // Construct shared application state
            let app_state = Arc::new(AppState {
                db_pool: pool.clone(),
                system,
                pending_devices: Arc::new(RwLock::new(HashMap::new())),
                settings,
                log_audit: Some(Arc::new(move |conn, actor, action, target, details| {
                    if let Err(e) = db::log_audit(conn, actor, action, target, details) {
                        log::error!("Audit logging failed: {:?}", e);
                    }
                    Ok(())
                })),
            });

            rocket.manage(app_state)
        }))

        // Register template rendering support
        .attach(Template::fairing())

        // Attach background maintenance fairings
        .attach(action_ttl::ActionTtlFairing)
        .attach(pending_cleanup::PendingCleanupFairing)

        // Record server startup after liftoff
        .attach(AdHoc::on_liftoff("Startup Audit", |rocket| {
            let app = rocket.state::<Arc<AppState>>().cloned();
            Box::pin(async move {
                if let Some(app) = app {
                    let mut conn = get_conn(&app.db_pool);
                    let _ = app.log_audit(
                        &mut conn,
                        "system",
                        "server_started",
                        None,
                        Some("PatchPilot server started"),
                    );
                }
            })
        }))

        // Mount API, auth, and UI routes
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::routes())
        .mount("/roles", routes::roles::routes())
        .mount("/settings", routes::settings::routes())
        .mount("/static", FileServer::from(relative!("static")))
        .mount("/", routes::page_routes())
}
