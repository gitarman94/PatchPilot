#[macro_use]
extern crate rocket;

use rocket::fs::{FileServer, relative};
use rocket::figment::{Figment, providers::{Env, Toml}};
use rocket::fairing::AdHoc;
use rocket_dyn_templates::Template;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::env;

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
    // Initialize logging
    flexi_logger::Logger::try_with_env_or_str(
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
    )
    .unwrap()
    .log_to_stdout()
    .start()
    .unwrap();

    // Figment configuration
    let figment = Figment::from(rocket::Config::default())
        .merge(Toml::file("Rocket.toml").nested())
        .merge(Env::prefixed("ROCKET_").global());

    // Detect systemd socket activation presence purely for logging
    let systemd_socket_active = std::env::var("LISTEN_FDS")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .map(|n| n > 0)
        .unwrap_or(false);

    if systemd_socket_active {
        log::info!("Systemd socket activation detected (LISTEN_FDS > 0). Rocket will inherit fd 3.");
    } else {
        log::info!("No systemd socket detected; Rocket will bind port from Rocket.toml or ROCKET_PORT.");
    }

    // Initialize DB pool
    let db_pool = initialize();

    // Load persistent settings into in-memory lock
    let settings = {
        let mut conn = get_conn(&db_pool);
        Arc::new(RwLock::new(settings::ServerSettings::load(&mut conn)))
    };

    // System state
    let system = SystemState::new(db_pool.clone());

    // Shared application state
    let app_state = Arc::new(AppState {
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
    });

    // Refresh metrics once at startup
    app_state.system.refresh();
    log::info!(
        "System memory: total {} MB, available {} MB",
        app_state.system.total_memory() / 1024 / 1024,
        app_state.system.available_memory() / 1024 / 1024
    );

    // Build Rocket normally. If systemd has passed the socket (fd 3), Rocket
    // will automatically use it — do not call .listen(...) or bind manually.
    rocket::custom(figment)
        .manage(db_pool)
        .manage(app_state.clone()) // manage Arc<AppState>
        .attach(Template::fairing())
        .attach(action_ttl::ActionTtlFairing)
        .attach(pending_cleanup::PendingCleanupFairing)
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
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::routes())
        .mount("/roles", routes::roles::routes())
        .mount("/settings", routes::settings::routes())
        .mount("/static", FileServer::from(relative!("static")))
        .mount("/", routes::page_routes())
}
