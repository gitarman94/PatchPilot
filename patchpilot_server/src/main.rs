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

use crate::db::{
    initialize,
    get_conn,
    create_default_admin,
    load_settings,
    DbPool,
};
use crate::action_ttl::spawn_action_ttl_task;
use crate::pending_cleanup::spawn_pending_cleanup;
use crate::state::{AppState, SystemState};
use crate::models::ServerSettings;
use crate::auth::AuthUser;

#[launch]
fn rocket() -> _ {
    // -----------------------------
    // Initialize DB + Logger
    // -----------------------------
    let pool: DbPool = initialize();

    {
        let mut conn = get_conn(&pool);
        if let Err(e) = create_default_admin(&mut conn) {
            eprintln!("Failed to create default admin: {}", e);
        }
    }

    // -----------------------------
    // Load Server Settings
    // -----------------------------
    let settings = {
        let mut conn = get_conn(&pool);
        let s = load_settings(&mut conn)
            .expect("Failed to load server settings");
        Arc::new(RwLock::new(s))
    };

    // -----------------------------
    // System + App State
    // -----------------------------
    let system_state = SystemState::new(pool.clone());

    let app_state = Arc::new(AppState {
        db_pool: pool.clone(),
        system: system_state.clone(),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: settings.clone(),

        // real audit logger wired to DB
        log_audit: Some(Arc::new(move |conn, actor, action, target, details| {
            let _ = crate::db::log_audit(
                conn,
                actor,
                action,
                target,
                details,
            );
        })),
    });

    // -----------------------------
    // Background Tasks
    // -----------------------------
    spawn_action_ttl_task(app_state.clone());
    spawn_pending_cleanup(app_state.clone());

    // -----------------------------
    // Startup Audit Event
    // -----------------------------
    {
        let mut conn = get_conn(&pool);
        let user = AuthUser {
            id: 1,
            username: "admin".to_string(),
            role: "Admin".to_string(),
        };
        user.audit(&mut conn, "server_started", None);
    }

    // -----------------------------
    // System Info Logging
    // -----------------------------
    info!(
        "System memory: total {} MB, available {} MB",
        app_state.system.total_memory() / 1024 / 1024,
        app_state.system.available_memory() / 1024 / 1024
    );

    info!("PatchPilot server ready");

    // -----------------------------
    // Rocket Build
    // -----------------------------
    rocket::build()
        .manage(pool)
        .manage(app_state)
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::users_groups_routes())
        .mount("/roles", routes::roles::roles_routes())
        .mount("/history", routes![routes::history::api_history])
        .mount("/audit", routes![routes::history::api_audit])
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
        .mount("/", routes::page_routes())
}
