#[macro_use] extern crate rocket;

mod db;
mod state;
mod routes;
mod tasks;
mod models;
mod schema;
mod settings;
mod auth; // RBAC + AuthUser

use crate::db::{initialize, get_conn, create_default_admin};
use crate::state::{AppState, SystemState};
use crate::tasks::{spawn_action_ttl_sweeper, spawn_pending_cleanup};

use rocket::fs::FileServer;
use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use sysinfo::System;
use log::info;

#[launch]
fn rocket() -> _ {
    // 1. Initialize DB + logger
    let pool = initialize();

    // 2. Create default admin user
    {
        let mut conn = get_conn(&pool);
        create_default_admin(&mut conn); // panics internally if fails
    }

    // 3. Spawn background tasks
    spawn_action_ttl_sweeper(pool.clone());

    // 4. Build AppState
    let system_state = SystemState {
        db_pool: pool.clone(),
        system: Arc::new(Mutex::new(System::new_all())),
    };

    let app_state = Arc::new(AppState {
        system: Arc::new(system_state),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: Arc::new(RwLock::new(settings::ServerSettings::load())),
    });

    spawn_pending_cleanup(app_state.clone());

    info!("Server ready");

    // 5. Rocket build
    rocket::build()
        .manage(pool)           // DB pool
        .manage(app_state)      // AppState with settings + system + pending devices
        .mount("/api", routes::api_routes())
        .mount("/", routes::page_routes())
        .mount("/auth", routes::auth_routes())              // login/logout
        .mount("/users-groups", routes::users_groups_routes()) // users & groups CRUD
        .mount("/roles", routes::roles_routes())            // roles CRUD
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
}
