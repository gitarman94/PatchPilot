// src/main.rs
#[macro_use]
extern crate rocket;

mod db;
mod routes;
mod models;
mod schema;
mod settings;
mod auth;
mod state;
mod action_ttl;
mod pending_cleanup;

use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use log::info;
use rocket::fs::FileServer;

use crate::db::{initialize, get_conn, DbPool};
use crate::action_ttl::spawn_action_ttl_task;
use crate::pending_cleanup::spawn_pending_cleanup;
use crate::state::{AppState, SystemState};
use crate::settings::ServerSettings;
use crate::auth::{AuthUser, UserRole};

#[launch]
fn rocket() -> _ {
    // Initialize DB + Logger
    let pool: DbPool = initialize();

    // Load Server Settings (app-level struct)
    let settings = {
        let mut conn = get_conn(&pool);
        let s = ServerSettings::load(&mut conn);
        Arc::new(RwLock::new(s))
    };

    // System + App State
    let system_state = SystemState::new(pool.clone());

    let app_state = Arc::new(AppState {
        db_pool: pool.clone(),
        system: system_state.clone(),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: settings.clone(),
        // real audit logger wired to DB; closure returns Diesel QueryResult<()>
        log_audit: Some(Arc::new(move |conn, actor, action, target, details| {
            crate::db::log_audit(conn, actor, action, target, details)
        })),
    });

    // Background Tasks
    spawn_action_ttl_task(app_state.clone());
    spawn_pending_cleanup(app_state.clone());

    // Spawn a small periodic task to cleanup stale pending devices so
    // AppState.pending_devices and cleanup_stale_devices are actually used.
    {
        let app_state_clone = app_state.clone();
        rocket::tokio::spawn(async move {
            loop {
                // Use server setting to choose max age; ensure a sensible minimum
                let max_age = {
                    let s = app_state_clone.settings.read().unwrap();
                    // Use auto_refresh_seconds as a reasonable cap for stale device expiry
                    let secs = s.auto_refresh_seconds.max(30);
                    secs as u64
                };

                app_state_clone.cleanup_stale_devices(max_age);

                rocket::tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        });
    }

    // Startup Audit Event (use an AuthUser record for the DB audit)
    {
        let mut conn = get_conn(&pool);
        let user = AuthUser {
            id: 1,
            username: "admin".to_string(),
            role: UserRole::Admin.as_str().to_string(),
        };
        let _ = user.audit(&mut conn, "server_started", None);
    }

    // Ensure the SystemState is refreshed before reading metrics (removes unused warning)
    app_state.system.refresh();

    // System Info Logging
    info!(
        "System memory: total {} MB, available {} MB",
        app_state.system.total_memory() / 1024 / 1024,
        app_state.system.available_memory() / 1024 / 1024
    );
    info!("PatchPilot server ready");

    // Rocket Build
    rocket::build()
        .manage(pool)
        .manage(app_state)
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::api_users_groups_routes())
        .mount("/roles", routes::roles::api_roles_routes())
        .mount("/history", routes![routes::history::api_history])
        .mount("/audit", routes![routes::history::api_audit])
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
        .mount("/", routes::page_routes())
}
