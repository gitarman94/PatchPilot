use rocket::{
    get, post,
    State,
    form::{Form, FromForm},
    http::Status,
};
use rocket_dyn_templates::Template;
use diesel::prelude::*;
use serde::Serialize;

use crate::state::AppState;
use crate::auth::AuthUser;
use crate::routes::history::log_audit;
use crate::db;
use crate::models::ServerSettings;

/// Form for updating settings
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

/// VIEW SETTINGS PAGE
#[get("/settings")]
pub async fn view_settings(
    state: &State<AppState>,
    _user: AuthUser,
) -> Result<Template, Status> {
    let pool = state.db_pool.clone();

    let settings_model: ServerSettings = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        db::load_settings(&mut conn).map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    let context = SettingsContext {
        settings: settings_model,
    };

    Ok(Template::render("settings", &context))
}

/// UPDATE SETTINGS
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

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };

        // Load existing settings
        let mut settings = match db::load_settings(&mut conn) {
            Ok(s) => s,
            Err(_) => return,
        };

        // Apply changes from form
        if let Some(v) = form.auto_approve_devices {
            settings.auto_approve_devices = v;
        }
        if let Some(v) = form.auto_refresh_enabled {
            settings.auto_refresh_enabled = v;
        }
        if let Some(v) = form.auto_refresh_seconds {
            settings.auto_refresh_seconds = v;
        }
        if let Some(v) = form.default_action_ttl_seconds {
            settings.default_action_ttl_seconds = v;
        }
        if let Some(v) = form.action_polling_enabled {
            settings.action_polling_enabled = v;
        }
        if let Some(v) = form.ping_target_ip {
            settings.ping_target_ip = v;
        }
        if let Some(v) = form.force_https {
            settings.force_https = v;
        }

        // Save updated settings to DB
        if db::save_settings(&mut conn, &settings).is_err() {
            return;
        }

        // Update shared in-memory settings
        if let Ok(mut guard) = shared_settings.write() {
            *guard = settings.clone();
        }

        // Audit log
        let _ = log_audit(
            &mut conn,
            &username,
            "update_settings",
            None,
            Some("Updated server settings"),
        );
    })
    .await
    .ok();

    Status::Ok
}

/// ROUTE MOUNTING
pub fn configure_routes(
    rocket: rocket::Rocket<rocket::Build>,
) -> rocket::Rocket<rocket::Build> {
    rocket.mount("/", routes![
        view_settings,
        update_settings,
    ])
}
