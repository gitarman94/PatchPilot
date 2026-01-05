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

use std::sync::{Arc, RwLock, Mutex};
use std::collections::HashMap;
use sysinfo::System;
use log::info;

use rocket::fs::FileServer;

use crate::db::{initialize, get_conn, create_default_admin, DbPool};
use crate::tasks::{spawn_action_ttl_sweeper, spawn_pending_cleanup};
use crate::state::{AppState, SystemState};

#[launch]
fn rocket() -> _ {
    // 1️⃣ Initialize DB + logging
    let pool: DbPool = initialize();

    // 2️⃣ Ensure default admin exists
    {
        let mut conn = get_conn(&pool);
        if let Err(e) = create_default_admin(&mut conn) {
            eprintln!("Failed to create default admin: {}", e);
        }
    }

    // 3️⃣ Spawn action TTL sweeper (background task)
    spawn_action_ttl_sweeper(pool.clone());

    // 4️⃣ Build SystemState
    let system_state = SystemState {
        db_pool: pool.clone(),
        system: Arc::new(Mutex::new(System::new_all())),
    };

    // 5️⃣ Build AppState
    let app_state = Arc::new(AppState {
        system: Arc::new(system_state),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: {
            let pool_clone = pool.clone();
            Arc::new(RwLock::new({
                let mut conn = get_conn(&pool_clone);
                settings::ServerSettings::load(&mut conn)
            }))
        },

    });

    // 6️⃣ Spawn pending device cleanup task
    spawn_pending_cleanup(app_state.clone());

    info!("PatchPilot server ready");

    // 7️⃣ Build Rocket
    rocket::build()
        // Shared state
        .manage(pool)          // Database connection pool
        .manage(app_state)     // Application-wide state

        // API endpoints
        .mount("/api", routes::api_routes())                // core API routes
        .mount("/auth", routes::auth_routes())             // authentication
        .mount("/users-groups", routes::users_groups_routes()) // user/group management
        .mount("/roles", routes::roles_routes())           // roles/permissions

        // Static assets
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))

        // Root-level pages and additional API routes
        .mount(
            "/",
            routes![
                routes::page_routes(),          // all HTML page handlers
                routes::history::api_history,  // history API
                routes::history::api_audit,    // audit API
                routes::system::system_info    // system info endpoint
            ]
        )


