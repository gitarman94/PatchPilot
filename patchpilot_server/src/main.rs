#[macro_use]
extern crate rocket;

mod db;
mod routes;
mod tasks;
mod models;
mod schema;
mod settings;
mod auth;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use rocket::fs::FileServer;
use sysinfo::System;
use log::info;

use crate::db::{initialize, get_conn, create_default_admin, DbPool};
use crate::tasks::{spawn_action_ttl_sweeper, spawn_pending_cleanup};

/// Global application state shared via Rocket managed state
pub struct AppState {
    pub system: Arc<Mutex<System>>,
    pub pending_devices: Mutex<HashMap<String, String>>,
    pub settings: Mutex<settings::ServerSettings>,
    pub db_pool: DbPool,
}

#[launch]
fn rocket() -> _ {
    // 1. Initialize database + logging
    let pool = initialize();

    // 2. Ensure default admin exists
    {
        let mut conn = get_conn(&pool);
        create_default_admin(&mut conn);
    }

    // 3. Spawn background tasks
    spawn_action_ttl_sweeper(pool.clone());

    // 4. Build application state
    let app_state = AppState {
        system: Arc::new(Mutex::new(System::new_all())),
        pending_devices: Mutex::new(HashMap::new()),
        settings: Mutex::new(settings::ServerSettings::load()),
        db_pool: pool.clone(),
    };

    spawn_pending_cleanup(Arc::new(app_state));

    info!("PatchPilot server ready");

    // 5. Rocket build
    rocket::build()
        .manage(pool)
        .manage(AppState {
            system: Arc::new(Mutex::new(System::new_all())),
            pending_devices: Mutex::new(HashMap::new()),
            settings: Mutex::new(settings::ServerSettings::load()),
            db_pool: pool,
        })
        .mount("/api", routes::api_routes())
        .mount("/", routes::page_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups_routes())
        .mount("/roles", routes::roles_routes())
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
}
