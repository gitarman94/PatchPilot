// src/routes/settings.rs
// src/routes/settings.rs
use rocket::{
    get, post, State, form::{Form, FromForm}, http::Status,
};
use rocket_dyn_templates::Template;
use serde::Serialize;
use crate::state::AppState;
use crate::auth::AuthUser;
use crate::db;
use crate::settings::ServerSettings;

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
    let settings_model: ServerSettings = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        // Use the app-level settings loader to convert DB row -> app struct
        Ok(ServerSettings::load(&mut conn))
    })
    .await
    .map_err(|_| Status::InternalServerError)? ?;
    let context = SettingsContext { settings: settings_model };
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
        // Load into the app-level ServerSettings struct
        let mut settings = ServerSettings::load(&mut conn);

        // Use the ServerSettings methods which persist changes inside the DB row
        if let Some(v) = form.auto_approve_devices {
            let _ = settings.set_auto_approve(&mut conn, v).map_err(|_| ())?;
        }
        if let Some(v) = form.auto_refresh_enabled {
            let _ = settings.set_auto_refresh(&mut conn, v).map_err(|_| ())?;
        }
        if let Some(v) = form.auto_refresh_seconds {
            let _ = settings.set_auto_refresh_interval(&mut conn, v).map_err(|_| ())?;
        }
        if let Some(v) = form.default_action_ttl_seconds {
            settings.default_action_ttl_seconds = v;
            // persist full row
            settings.save(&mut conn);
        }
        if let Some(v) = form.action_polling_enabled {
            settings.action_polling_enabled = v;
            settings.save(&mut conn);
        }
        if let Some(v) = form.ping_target_ip {
            settings.ping_target_ip = v;
            settings.save(&mut conn);
        }
        if let Some(v) = form.force_https {
            settings.force_https = v;
            settings.save(&mut conn);
        }

        // Update in-memory shared settings if possible
        if let Ok(mut guard) = shared_settings.write() {
            *guard = settings.clone();
        }

        // Log audit using synchronous DB log helper
        let _ = db::log_audit(&mut conn, &username, "update_settings", None, Some("Updated server settings"));
        Ok(())
    })
    .await;

    match result {
        Ok(Ok(_)) => Status::Ok,
        _ => Status::InternalServerError,
    }
}
