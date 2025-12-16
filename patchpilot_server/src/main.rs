#[macro_use] extern crate rocket;

mod db;
mod state;
mod routes;
mod tasks;
mod models;
mod schema;
mod settings;

use crate::db::pool::init_pool;
use crate::db::init::initialize_db;
use crate::state::AppState;
use crate::tasks::{spawn_action_ttl_sweeper, spawn_pending_cleanup};

use rocket::fs::FileServer;
use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use sysinfo::System;
use log::info;

#[launch]
fn rocket() -> _ {
    db::logger::init_logger();

    let pool = init_pool();
    {
        let mut conn = pool.get().expect("DB connect failed");
        initialize_db(&mut conn).expect("DB init failed");
    }

    spawn_action_ttl_sweeper(pool.clone());

    let app_state = Arc::new(AppState {
        system: Mutex::new(System::new_all()),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: Arc::new(RwLock::new(settings::ServerSettings::load())),
    });

    spawn_pending_cleanup(app_state.clone());

    info!("Server ready");

    rocket::build()
        .manage(pool)
        .manage(app_state)
        .mount("/api", routes::api_routes())
        .mount("/", routes::page_routes())
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
}
