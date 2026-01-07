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

use crate::db::{initialize, get_conn, create_default_admin, DbPool};
use crate::action_ttl::spawn_action_ttl_task;
use crate::pending_cleanup::spawn_pending_cleanup;
use crate::state::{AppState, SystemState};
use crate::models::{ServerSettings as ModelServerSettings, AuditLog};
use crate::auth::AuthUser;
use diesel::RunQueryDsl;

type AuditClosure =
    dyn Fn(&mut diesel::SqliteConnection, &str, &str, Option<&str>, Option<&str>) + Send + Sync;

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

    // 3. Load server settings
    let server_settings = {
        let mut conn = get_conn(&pool);
        let settings = settings::ServerSettings::load(&mut conn);

        Arc::new(RwLock::new(ModelServerSettings {
            id: 0,
            auto_approve_devices: settings.auto_approve_devices,
            auto_refresh_enabled: settings.auto_refresh_enabled,
            auto_refresh_seconds: settings.auto_refresh_seconds,
            default_action_ttl_seconds: settings.default_action_ttl_seconds,
            action_polling_enabled: settings.action_polling_enabled,
            ping_target_ip: settings.ping_target_ip,
            // force_https and allow_http removed
        }))
    };

    // 4. Build SystemState
    let system_state = SystemState::new(pool.clone());

    // 5. Build AppState
    let app_state = Arc::new(AppState {
        system: Arc::new(system_state),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: server_settings.clone(),
        // db_pool and audit removed from AppState
    });

    // 6. Spawn background tasks
    spawn_action_ttl_task(app_state.clone());
    spawn_pending_cleanup(app_state.clone());

    // 7. Example usage of AuthUser::audit
    {
        let mut conn = get_conn(&pool);
        let demo_user = AuthUser { id: 1, username: "admin".into(), role: "Admin".into() };
        demo_user.audit(&mut conn, "server_started", None);
    }

    // 8. Log system memory info
    info!(
        "System memory: total {} MB, available {} MB",
        app_state.system.total_memory() / 1024 / 1024,
        app_state.system.available_memory() / 1024 / 1024
    );

    info!("PatchPilot server ready");

    // 9. HTTP/HTTPS setup
    let settings_read = app_state.settings.read().unwrap();

    let rocket_builder = rocket::build()
        .manage(pool)
        .manage(app_state)
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups_routes())
        .mount("/roles", routes::roles_routes())
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
        .mount("/", routes::page_routes())
        .mount("/history", routes![routes::history::api_history])
        .mount("/audit", routes![routes::history::api_audit]);

    // Use TLS if requested and feature enabled
    #[cfg(feature = "tls")]
    {
        use rocket::config::{Config, TlsConfig};
        let figment = Config::figment()
            .merge(("tls", TlsConfig::from_paths("certs/server.crt", "certs/server.key")));
        return rocket::custom(figment)
            .mount("/api", routes::api_routes())
            .mount("/auth", routes::auth_routes())
            .mount("/users-groups", routes::users_groups_routes())
            .mount("/roles", routes::roles_routes())
            .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
            .mount("/", routes::page_routes())
            .mount("/history", routes![routes::history::api_history])
            .mount("/audit", routes![routes::history::api_audit]);
    }

    // Default: return the regular Rocket builder
    rocket_builder
}
