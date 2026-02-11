// src/main.rs
#[macro_use]
extern crate rocket;

use rocket::fs::{FileServer, relative};
use rocket::figment::providers::{Env, Toml, Format};
use rocket::fairing::AdHoc;
use rocket_dyn_templates::Template;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::env;
use std::net::TcpListener;
use std::process;

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
        env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
    )
    .unwrap()
    .log_to_stdout()
    .start()
    .unwrap();

    // Determine address/port from environment (or Rocket.toml)
    let address = env::var("ROCKET_ADDRESS").unwrap_or_else(|_| "0.0.0.0".into());
    let port = env::var("ROCKET_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse::<u16>()
        .unwrap_or(8080);

    // Quick pre-bind check: try to bind to the address/port to ensure availability.
    // If it's already in use, exit with non-zero so systemd / supervisor sees the failure.
    if let Err(e) = TcpListener::bind((address.as_str(), port)) {
        log::error!("Port {} on {} is not available: {:?}", port, address, e);
        process::exit(1);
    }

    log::info!("[*] Rocket will bind to {}:{}.", address, port);

    // Build figment: merge Rocket.toml and environment, and override address/port
    let override_map = serde_json::json!({
        "address": address,
        "port": port
    });

    let figment = rocket::Config::figment()
        .merge(Toml::file("Rocket.toml"))
        .merge(Env::prefixed("ROCKET_").global())
        .merge(rocket::figment::providers::Serialized::from(override_map, "default"))
        .merge(("template_dir", "/opt/patchpilot_server/templates"));

    // Initialize DB pool and settings
    let db_pool = initialize();

    let settings = {
        let mut conn = get_conn(&db_pool);
        Arc::new(RwLock::new(settings::ServerSettings::load(&mut conn)))
    };

    let system = SystemState::new(db_pool.clone());

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

    app_state.system.refresh();
    log::info!(
        "System memory: total {} MB, available {} MB",
        app_state.system.total_memory() / 1024 / 1024,
        app_state.system.available_memory() / 1024 / 1024
    );

// src/main.rs
#[macro_use]
extern crate rocket;

use rocket::fs::{FileServer, relative};
use rocket::figment::providers::{Env, Toml, Format};
use rocket::fairing::AdHoc;
use rocket_dyn_templates::Template;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::env;
use std::net::TcpListener;
use std::process;

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
        env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
    )
    .unwrap()
    .log_to_stdout()
    .start()
    .unwrap();

    // Determine address/port from environment (or Rocket.toml)
    let address = env::var("ROCKET_ADDRESS").unwrap_or_else(|_| "0.0.0.0".into());
    let port = env::var("ROCKET_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse::<u16>()
        .unwrap_or(8080);

    // Quick pre-bind check: try to bind to the address/port to ensure availability.
    // If it's already in use, exit with non-zero so systemd / supervisor sees the failure.
    if let Err(e) = TcpListener::bind((address.as_str(), port)) {
        log::error!("Port {} on {} is not available: {:?}", port, address, e);
        process::exit(1);
    }

    log::info!("[*] Rocket will bind to {}:{}.", address, port);

    // Build figment: merge Rocket.toml and environment, and override address/port
    let override_map = serde_json::json!({
        "address": address,
        "port": port
    });

    let figment = rocket::Config::figment()
        .merge(Toml::file("Rocket.toml"))
        .merge(Env::prefixed("ROCKET_").global())
        .merge(rocket::figment::providers::Serialized::from(override_map, "default"))
        .merge(("template_dir", "/opt/patchpilot_server/templates"));

    // Initialize DB pool and settings
    let db_pool = initialize();

    let settings = {
        let mut conn = get_conn(&db_pool);
        Arc::new(RwLock::new(settings::ServerSettings::load(&mut conn)))
    };

    let system = SystemState::new(db_pool.clone());

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

    app_state.system.refresh();
    log::info!(
        "System memory: total {} MB, available {} MB",
        app_state.system.total_memory() / 1024 / 1024,
        app_state.system.available_memory() / 1024 / 1024
    );

    rocket::custom(figment)
        .manage(db_pool)
        .manage(app_state.clone())
        .attach(Template::fairing())
        .attach(action_ttl::ActionTtlFairing)
        .attach(pending_cleanup::PendingCleanupFairing)
        .attach(AdHoc::on_liftoff("Startup Audit", |rocket| {
            let app = rocket.state::<Arc<AppState>>().cloned();
            Box::pin(async move {
                if let Some(app) = app {
                    let app = app.clone();
                    tokio::spawn(async move {
                        let res = tokio::task::spawn_blocking(move || {
                            let mut conn = get_conn(&app.db_pool);
                            let _ = app.log_audit(
                                &mut conn,
                                "system",
                                "server_started",
                                None,
                                Some("PatchPilot server started"),
                            );
                        })
                        .await;
                        if let Err(e) = res {
                            log::error!("Startup audit spawn_blocking failed: {:?}", e);
                        }
                    });
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
}

