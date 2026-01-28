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

    // Build Rocket configuration
    let figment = Figment::from(rocket::Config::default())
        .merge(Toml::file("Rocket.toml").nested())
        .merge(Env::prefixed("ROCKET_").global());

    rocket::custom(figment)
        // Manage DB pool
        .manage(initialize())

        // Initialize AppState after Rocket ignition (so logging is active)
        .attach(AdHoc::on_ignite("Init AppState", |rocket| async move {
            let pool = rocket.state::<DbPool>().unwrap().clone();

            // Load server settings from DB
            let settings = {
                let mut conn = get_conn(&pool);
                Arc::new(RwLock::new(settings::ServerSettings::load(&mut conn)))
            };

            // Initialize system state (returns Arc<SystemState>)
            let system = SystemState::new(pool.clone());

            // Construct shared application state
            let app_state = Arc::new(AppState {
                db_pool: pool.clone(),
                system: system.clone(),
                pending_devices: Arc::new(RwLock::new(HashMap::new())),
                settings: settings.clone(),
                log_audit: Some(Arc::new(move |conn, actor, action, target, details| {
                    if let Err(e) = db::log_audit(conn, actor, action, target, details) {
                        log::error!("Audit logging failed: {:?}", e);
                    }
                    Ok(())
                })),
            });

            // Refresh system metrics and log memory (uses SystemState methods)
            app_state.system.refresh();
            log::info!(
                "System memory: total {} MB, available {} MB",
                app_state.system.total_memory() / 1024 / 1024,
                app_state.system.available_memory() / 1024 / 1024
            );

            rocket.manage(app_state)
        }))

        // Templates
        .attach(Template::fairing())

        // Background fairings
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

        // Routes
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::routes())
        .mount("/roles", routes::roles::routes())
        .mount("/settings", routes::settings::routes())
        .mount("/static", FileServer::from(relative!("static")))
        .mount("/", routes::page_routes())
}
