#[macro_use]
extern crate rocket;

mod db;
mod routes;
mod tasks;
mod models;
mod schema;
mod settings;
mod auth;
mod state;

use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use log::info;

use rocket::fs::FileServer;

use crate::db::{initialize, get_conn, create_default_admin, DbPool};
use crate::tasks::{spawn_action_ttl_task, spawn_pending_cleanup};
use crate::state::{AppState, SystemState};
use crate::settings::ServerSettings;
use crate::auth::AuthUser;

#[launch]
fn rocket() -> _ {
    // 1. Initialize DB + logging
    let pool: DbPool = initialize();

    // 2. Ensure default admin exists
    {
        let mut conn = get_conn(&pool);
        if let Err(e) = create_default_admin(&mut conn) {
            eprintln!("Failed to create default admin: {}", e);
        }
    }

    // 3. Load and initialize server settings
    let server_settings = {
        let mut conn = get_conn(&pool);
        let mut settings = ServerSettings::load(&mut conn);

        // Call setters to remove dead code warnings
        let _ = settings.set_auto_approve(&mut conn, settings.auto_approve_devices);
        let _ = settings.set_auto_refresh(&mut conn, settings.auto_refresh_enabled);
        let _ = settings.set_auto_refresh_interval(&mut conn, settings.auto_refresh_seconds);

        settings.save(&mut conn);

        Arc::new(RwLock::new(settings))
    };

    // 4. Spawn action TTL task (background task)
    spawn_action_ttl_task(Arc::new(AppState {
        system: Arc::new(SystemState::new(pool.clone())),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: server_settings.clone(),
        db_pool: pool.clone(),
        log_audit: None, // optionally set a closure if you have audit logging
    }));

    // 5. Build SystemState
    let system_state = SystemState::new(pool.clone());

    // 6. Build AppState
    let app_state = Arc::new(AppState {
        system: Arc::new(system_state),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: server_settings.clone(),
        db_pool: pool.clone(),
        log_audit: None,
    });

    // 7. Spawn pending device cleanup task
    spawn_pending_cleanup(app_state.clone());

    // 8. Example usage of AuthUser::audit
    {
        let mut conn = get_conn(&pool);
        let demo_user = AuthUser { id: 1, username: "admin".into(), role: "Admin".into() };
        demo_user.audit(&mut conn, "server_started", None);
    }

    // 9. Log system memory info
    info!(
        "System memory: total {} MB, available {} MB",
        app_state.system.total_memory() / 1024 / 1024,
        app_state.system.available_memory() / 1024 / 1024
    );

    info!("PatchPilot server ready");

    // 10. Build Rocket
    rocket::build()
        .manage(pool)
        .manage(app_state)
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups_routes())
        .mount("/roles", routes::roles_routes())
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
        .mount("/", routes::page_routes())
        .mount("/history", routes![routes::history::api_history])
        .mount("/audit", routes![routes::history::api_audit])
}
