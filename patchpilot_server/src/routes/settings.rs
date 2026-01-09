// src/routes/settings.rs
use rocket::{get, post, State, form::{Form, FromForm}, http::Status};
use rocket_dyn_templates::Template;
use serde::Serialize;

use crate::state::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::settings::ServerSettings; // now re-exported publicly by src/settings.rs

#[derive(FromForm)]
pub struct ServerSettingsForm {
    pub auto_approve_devices: Option<bool>,
    pub auto_refresh_enabled: Option<bool>,
    pub auto_refresh_seconds: Option<i64>,
    pub default_action_ttl_seconds: Option<i64>,
    pub action_polling_enabled: Option<bool>,
    pub ping_target_ip: Option<String>,
    pub force_https: Option<bool>,
}

#[derive(Serialize)]
struct SettingsContext {
    settings: ServerSettings,
}

/// View settings page
#[get("/settings")]
pub async fn view_settings(
    state: &State<AppState>,
    _user: AuthUser,
) -> Result<Template, Status> {
    let pool = state.db_pool.clone();

    let settings_model: ServerSettings = rocket::tokio::task::spawn_blocking(move || -> Result<ServerSettings, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        // Use the app-level settings loader to convert DB row -> app struct
        Ok(ServerSettings::load(&mut conn))
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    let context = SettingsContext {
        settings: settings_model,
    };

    Ok(Template::render("settings", &context))
}

/// Update settings
#[post("/settings/update", data = "<form>")]
pub async fn update_settings(
    state: &State<AppState>,
    form: Form<ServerSettingsForm>,
    user: AuthUser,
) -> Status {
    let username = user.username.clone();
    let form = form.into_inner();
    let pool = state.db_pool.clone();
    let shared_settings = state.settings.clone();

    let result = rocket::tokio::task::spawn_blocking(move || -> Result<(), ()> {
        let mut conn = pool.get().map_err(|_| ())?;
        // Load DB row
        let mut row = db::load_settings(&mut conn).map_err(|_| ())?;

        // Update fields from form where provided
        if let Some(v) = form.auto_approve_devices {
            row.auto_approve_devices = v;
        }
        if let Some(v) = form.auto_refresh_enabled {
            row.auto_refresh_enabled = v;
        }
        if let Some(v) = form.auto_refresh_seconds {
            row.auto_refresh_seconds = v;
        }
        if let Some(v) = form.default_action_ttl_seconds {
            row.default_action_ttl_seconds = v;
        }
        if let Some(v) = form.action_polling_enabled {
            row.action_polling_enabled = v;
        }
        if let Some(v) = form.ping_target_ip {
            row.ping_target_ip = v;
        }
        if let Some(v) = form.force_https {
            row.force_https = v;
        }

        // Persist row
        db::save_settings(&mut conn, &row).map_err(|_| ())?;

        // Mirror into shared in-memory ServerSettings (AppState.settings)
        let settings_struct = ServerSettings {
            id: row.id,
            auto_approve_devices: row.auto_approve_devices,
            auto_refresh_enabled: row.auto_refresh_enabled,
            auto_refresh_seconds: row.auto_refresh_seconds,
            default_action_ttl_seconds: row.default_action_ttl_seconds,
            action_polling_enabled: row.action_polling_enabled,
            ping_target_ip: row.ping_target_ip.clone(),
            force_https: row.force_https,
        };

        if let Ok(mut guard) = shared_settings.write() {
            *guard = settings_struct.clone();
        }

        // Log audit using synchronous DB helper
        let _ = db::log_audit(&mut conn, &username, "update_settings", None, Some("Updated server settings"));
        Ok(())
    })
    .await;

    match result {
        Ok(Ok(_)) => Status::Ok,
        _ => Status::InternalServerError,
    }
}

pub fn configure_routes(rocket: rocket::Rocket<rocket::Build>) -> rocket::Rocket<rocket::Build> {
    rocket.mount("/", routes![view_settings, update_settings])
}
