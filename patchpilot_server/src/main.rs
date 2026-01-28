#[macro_use]
extern crate rocket;

use rocket::fs::{FileServer, relative};
use rocket_dyn_templates::Template;

mod schema;
mod db;
mod models;
mod auth;
mod state;
mod settings;
mod routes;
mod action_ttl;
mod pending_cleanup;

use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use db::{DbPool, initialize, get_conn};
use state::{SystemState, AppState};
use rocket::figment::providers::{Env, Toml};

#[launch]
fn rocket() -> _ {
    // Initialize DB + Logger
    let pool: DbPool = initialize();

    // Load server settings
    let settings = {
        let mut conn = get_conn(&pool);
        let s = settings::ServerSettings::load(&mut conn);
        Arc::new(RwLock::new(s))
    };

    // Initialize system state
    let system_state = SystemState::new(pool.clone());

    // Initialize app state
    let app_state = Arc::new(AppState {
        db_pool: pool.clone(),
        system: system_state.clone(),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: settings.clone(),
        log_audit: Some(Arc::new(move |conn, actor, action, target, details| {
            if let Err(e) = db::log_audit(conn, actor, action, target, details) {
                log::error!("Audit logging failed: {:?}", e);
            }
            Ok(())
        })),
    });

    // Log server start audit (best-effort)
    {
        let mut conn = get_conn(&pool);
        if let Err(e) = app_state.log_audit(
            &mut conn,
            "system",
            "server_started",
            None,
            Some("PatchPilot server started"),
        ) {
            log::error!("Failed to log server start audit: {:?}", e);
        }
    }

    // Refresh system metrics
    app_state.system.refresh();
    log::info!(
        "System memory: total {} MB, available {} MB",
        app_state.system.total_memory() / 1024 / 1024,
        app_state.system.available_memory() / 1024 / 1024
    );
    log::info!("PatchPilot server ready");

    // Rocket figment: merge Rocket.toml + env vars
    let figment = rocket::Config::figment()
        .merge(Toml::file("Rocket.toml"))
        .merge(Env::prefixed("ROCKET_"));

    rocket::custom(figment)
        .manage(pool)
        .manage(app_state)
        .attach(Template::fairing())
        .attach(action_ttl::ActionTtlFairing)
        .attach(pending_cleanup::PendingCleanupFairing)
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::routes())
        .mount("/roles", routes::roles::routes())
        .mount("/settings", routes::settings::routes())
        .mount("/static", FileServer::from(relative!("static")))
        .mount("/", routes::page_routes())
}
