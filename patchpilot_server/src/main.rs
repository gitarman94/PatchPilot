// bring Rocket procedural macros into scope
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
use auth::AuthUser;
use models::ServerSettings;
use state::{SystemState, AppState};

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

    // Spawn background tasks
    action_ttl::spawn_action_ttl_task(app_state.clone());
    pending_cleanup::spawn_pending_cleanup(app_state.clone());

    // Periodic cleanup of stale devices
    {
        let app_state_clone = app_state.clone();
        rocket::tokio::spawn(async move {
            loop {
                let max_age = app_state_clone
                    .settings
                    .read()
                    .map(|s| s.auto_refresh_seconds)
                    .unwrap_or(30)
                    .max(30) as u64;
                app_state_clone.cleanup_stale_devices(max_age);
                rocket::tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        });
    }

    // Log server start audit (best-effort)
    {
        let mut conn = get_conn(&pool);
        let user = AuthUser {
            id: 1,
            username: "admin".to_string(),
            role: "Admin".to_string(),
        };
        if let Err(e) = user.audit(&mut conn, "server_started", None) {
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

    // Rocket server build: static files, API, and templates
    rocket::build()
        .attach(Template::fairing())
        .manage(pool)
        .manage(app_state)
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::api_users_groups_routes())
        .mount("/roles", routes::roles::api_roles_routes())
        .mount("/history", rocket::routes![routes::history::api_history])
        .mount("/audit", rocket::routes![routes::history::api_audit])
        .mount("/static", FileServer::from(relative!("static")))
        .mount("/", routes::page_routes())
}
