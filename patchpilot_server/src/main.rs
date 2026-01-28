// src/main.rs
#[macro_use]
extern crate rocket;

use rocket::fs::{FileServer, relative};
use rocket::figment::{
    Figment,
    providers::{Env, Toml, Format},
};
use rocket_dyn_templates::Template;
use rocket::fairing::AdHoc;

use std::sync::{Arc, RwLock};
use std::collections::HashMap;

mod schema;
mod db;
mod models;
mod auth;
mod state;
mod settings;
mod routes;
mod action_ttl;
mod pending_cleanup;

use db::{DbPool, initialize, get_conn};
use state::{SystemState, AppState};

/// Entrypoint for Rocket
#[launch]
fn rocket() -> _ {
    // Initialize logger. RUST_LOG must be provided by the environment (e.g. systemd EnvironmentFile).
    flexi_logger::Logger::try_with_env_or_str(
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
    )
    .unwrap()
    .log_to_stdout()
    .start()
    .unwrap();

    // Rocket Figment configuration (Toml + ROCKET_* env vars provided externally)
    let figment = Figment::from(rocket::Config::default())
        .merge(Toml::file("Rocket.toml").nested())
        .merge(Env::prefixed("ROCKET_").global());

    // Create Rocket with DB pool managed early (no in-process .env mutation).
    rocket::custom(figment)
        // Initialize and manage DB pool immediately so later fairings/ignite code can access it.
        .manage(initialize())
        // Defer heavy or risky app initialization to an on_ignite AdHoc fairing so Rocket's logging is active.
        .attach(AdHoc::on_ignite("Init AppState", |rocket| {
            Box::pin(async move {
                // Grab the DB pool that we managed above.
                let pool = rocket.state::<DbPool>()
                    .expect("DB pool must be managed")
                    .clone();

                // Load server settings from DB (may touch DB; do it while Rocket logging is available).
                let settings = {
                    let mut conn = get_conn(&pool);
                    Arc::new(RwLock::new(settings::ServerSettings::load(&mut conn)))
                };

                // Build system state (may perform non-trivial initialization).
                let system = SystemState::new(pool.clone());

                // Build AppState and attach an audit-logging closure.
                let app_state = Arc::new(AppState {
                    db_pool: pool.clone(),
                    system,
                    pending_devices: Arc::new(RwLock::new(HashMap::new())),
                    settings: settings.clone(),
                    log_audit: Some(Arc::new(move |conn, actor, action, target, details| {
                        if let Err(e) = db::log_audit(conn, actor, action, target, details) {
                            log::error!("Audit logging failed: {:?}", e);
                        }
                        Ok(())
                    })),
                });

                // Manage the AppState so subsequent fairings/routes can access it.
                Ok(rocket.manage(app_state))
            })
        }))
        // Attach template fairing (safe after AppState management).
        .attach(Template::fairing())
        // Attach fairings that may depend on managed AppState. Since AdHoc above runs first,
        // these fairings will see the managed AppState during their on_ignite/on_liftoff as expected.
        .attach(action_ttl::ActionTtlFairing)
        .attach(pending_cleanup::PendingCleanupFairing)
        // Attach an on_liftoff AdHoc to record the "server_started" audit once Rocket is fully up.
        .attach(AdHoc::on_liftoff("Startup Audit", |rocket| {
            // Capture a clone of the AppState handle (if present) synchronously, then run audit async.
            let maybe_app = rocket.state::<Arc<AppState>>().cloned();
            Box::pin(async move {
                if let Some(app_state) = maybe_app {
                    // Perform audit logging safely now that Rocket is live.
                    let pool = app_state.db_pool.clone();
                    let mut conn = get_conn(&pool);
                    if let Err(e) = app_state.log_audit(
                        &mut conn,
                        "system",
                        "server_started",
                        None,
                        Some("PatchPilot server started"),
                    ) {
                        log::error!("Failed to log server start audit: {:?}", e);
                    }
                } else {
                    log::warn!("AppState not available on liftoff; skipping startup audit");
                }
            })
        }))
        // Routes and static files
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::routes())
        .mount("/roles", routes::roles::routes())
        .mount("/settings", routes::settings::routes())
        .mount("/static", FileServer::from(relative!("static")))
        .mount("/", routes::page_routes())
}
