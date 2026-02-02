#[macro_use]
extern crate rocket;

use rocket::fs::{FileServer, relative};
use rocket::figment::providers::{Env, Toml};
use rocket::fairing::AdHoc;
use rocket_dyn_templates::Template;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::env;
use std::os::unix::io::FromRawFd;

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
    // Detect presence of systemd socket activation
    let systemd_socket_active = env::var("LISTEN_FDS")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .map(|n| n > 0)
        .unwrap_or(false);

    // Initialize logging
    flexi_logger::Logger::try_with_env_or_str(
        env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
    )
    .unwrap()
    .log_to_stdout()
    .start()
    .unwrap();

    if systemd_socket_active {
        log::info!("[*] Systemd socket detected; Rocket will inherit fd 3. No port override needed.");
    } else {
        log::error!("[*] No systemd socket detected; Rocket will not bind its own port. Exiting.");
    }

    // Merge Rocket configuration (templates dir preserved)
    let figment = rocket::Config::figment()
        .merge(Toml::file("Rocket.toml"))
        .merge(Env::prefixed("ROCKET_").global())
        .merge(("template_dir", "/opt/patchpilot_server/templates"));

    // Initialize DB and app state (unchanged)
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

    // Enforce that systemd socket activation must be present.
    // If not present, exit immediately (we do not want Rocket to bind on its own).
    if !systemd_socket_active {
        // Exit with non-zero so systemd reports a failure in the journal.
        std::process::exit(1);
    }

    // Convert systemd-passed fd 3 into a Tokio listener and use Rocket's `.listen(...)`.
    // Safety: systemd hands us ownership of the fd; using from_raw_fd is appropriate here.
    let std_listener = unsafe { std::net::TcpListener::from_raw_fd(3) };
    let listener = rocket::tokio::net::TcpListener::from_std(std_listener)
        .expect("Failed to convert systemd fd to Tokio listener");

    // Build Rocket and use the provided listener.
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
                        let app = app;
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
        .listen(listener)
}
