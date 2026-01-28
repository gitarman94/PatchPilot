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
use std::{fs, path::Path};

use db::{DbPool, initialize, get_conn};
use state::{SystemState, AppState};
use rocket::figment::{Figment, providers::{Env, Toml, Format}};

fn load_env_file<P: AsRef<Path>>(path: P) {
    let path = path.as_ref();
    if !path.exists() { return; }
    if let Ok(contents) = fs::read_to_string(path) {
        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            let mut parts = line.splitn(2, '=');
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                let mut v = value.trim().to_string();
                if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
                    v = v[1..v.len()-1].to_string();
                }
                std::env::set_var(key.trim(), v);
            }
        }
    }
}

#[launch]
fn rocket() -> _ {
    let _ = env_logger::try_init();

    load_env_file("/opt/patchpilot_server/.env");

    if std::env::var("ROCKET_PROFILE").is_err() {
        std::env::set_var("ROCKET_PROFILE", "dev");
    }
    if std::env::var("ROCKET_ADDRESS").is_err() {
        std::env::set_var("ROCKET_ADDRESS", "0.0.0.0");
    }
    if std::env::var("ROCKET_PORT").is_err() {
        std::env::set_var("ROCKET_PORT", "8080");
    }
    if std::env::var("HOME").is_err() {
        std::env::set_var("HOME", "/home/patchpilot");
    }

    let pool: DbPool = initialize();

    let settings = {
        let mut conn = get_conn(&pool);
        Arc::new(RwLock::new(settings::ServerSettings::load(&mut conn)))
    };

    let system_state = SystemState::new(pool.clone());

    let app_state = Arc::new(AppState {
        db_pool: pool.clone(),
        system: system_state.clone(),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: settings.clone(),
        log_audit: Some(Arc::new(move |conn, actor, action, target, details| {
            let _ = db::log_audit(conn, actor, action, target, details);
            Ok(())
        })),
    });

    {
        let mut conn = get_conn(&pool);
        let _ = app_state.log_audit(
            &mut conn,
            "system",
            "server_started",
            None,
            Some("PatchPilot server started"),
        );
    }

    app_state.system.refresh();

    let figment = Figment::from(rocket::Config::default())
        .merge(Toml::file("Rocket.toml").nested())
        .merge(Env::prefixed("ROCKET_"));

    rocket::custom(figment)
        .attach(Template::fairing())
        .attach(action_ttl::ActionTtlFairing)
        .attach(pending_cleanup::PendingCleanupFairing)
        .manage(pool)
        .manage(app_state)
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::routes())
        .mount("/roles", routes::roles::routes())
        .mount("/settings", routes::settings::routes())
        .mount("/static", FileServer::from(relative!("static")))
        .mount("/", routes::page_routes())
}
