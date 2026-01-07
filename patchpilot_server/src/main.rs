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

    // 4. Build SystemState
    let system_state = SystemState::new(pool.clone());

    // 5. Build AppState
    let app_state = Arc::new(AppState {
        system: Arc::new(system_state),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: server_settings.clone(),
        db_pool: pool.clone(),
        log_audit: Some(Arc::new(|conn, actor, action_type, target, details| {
            // Example audit logging implementation
            use crate::models::AuditLog;
            use diesel::prelude::*;
            let log = AuditLog {
                id: 0,
                actor: actor.to_string(),
                action_type: action_type.to_string(),
                target: target.map(|s| s.to_string()),
                details: details.map(|s| s.to_string()),
                created_at: chrono::Utc::now().naive_utc(),
            };
            diesel::insert_into(crate::schema::audit::table)
                .values(&log)
                .execute(conn)
                .ok();
        })),
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

    // 9. HTTP/HTTPS handling
    let settings_read = app_state.settings.read().unwrap();
    let force_https = settings_read.force_https();
    let allow_http = settings_read.allow_http();

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

    if force_https {
        // TLS mode: provide server.crt & server.key
        rocket_builder.configure(rocket::Config {
            tls: Some(rocket::config::TlsConfig::from_paths("certs/server.crt", "certs/server.key")),
            ..Default::default()
        })
    } else if allow_http {
        rocket_builder // default HTTP
    } else {
        rocket_builder // fallback
    }
}
