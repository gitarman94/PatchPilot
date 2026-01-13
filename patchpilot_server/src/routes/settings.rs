use rocket::{get, post, State, form::{Form, FromForm}, http::Status};
use rocket_dyn_templates::Template;
use serde::Serialize;

use crate::state::AppState;
use crate::auth::AuthUser;
use crate::db::{self, ServerSettingsRow}; // import ServerSettingsRow from db

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
    settings: crate::settings::ServerSettings, // still use your in-memory ServerSettings
}

#[get("/settings")]
pub async fn view_settings(
    state: &State<AppState>,
    _user: AuthUser,
) -> Result<Template, Status> {
    let pool = state.db_pool.clone();

    let settings_model: crate::settings::ServerSettings =
        rocket::tokio::task::spawn_blocking(move || -> Result<_, Status> {
            let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
            let row: ServerSettingsRow = db::load_settings(&mut conn)
                .map_err(|_| Status::InternalServerError)?;

            Ok(crate::settings::ServerSettings {
                id: row.id,
                auto_approve_devices: row.auto_approve_devices,
                auto_refresh_enabled: row.auto_refresh_enabled,
                auto_refresh_seconds: row.auto_refresh_seconds,
                default_action_ttl_seconds: row.default_action_ttl_seconds,
                action_polling_enabled: row.action_polling_enabled,
                ping_target_ip: row.ping_target_ip,
                force_https: row.force_https,
            })
        })
        .await
        .map_err(|_| Status::InternalServerError)??;

    let context = SettingsContext {
        settings: settings_model,
    };

    Ok(Template::render("settings", &context))
}

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

    let result = rocket::tokio::task::spawn_blocking(move || -> Result<(), Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let mut row: ServerSettingsRow = db::load_settings(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        if let Some(v) = form.auto_approve_devices { row.auto_approve_devices = v; }
        if let Some(v) = form.auto_refresh_enabled { row.auto_refresh_enabled = v; }
        if let Some(v) = form.auto_refresh_seconds { row.auto_refresh_seconds = v; }
        if let Some(v) = form.default_action_ttl_seconds { row.default_action_ttl_seconds = v; }
        if let Some(v) = form.action_polling_enabled { row.action_polling_enabled = v; }
        if let Some(v) = form.ping_target_ip { row.ping_target_ip = v; }
        if let Some(v) = form.force_https { row.force_https = v; }

        db::save_settings(&mut conn, &row)
            .map_err(|_| Status::InternalServerError)?;

        let settings_struct = crate::settings::ServerSettings {
            id: row.id,
            auto_approve_devices: row.auto_approve_devices,
            auto_refresh_enabled: row.auto_refresh_enabled,
            auto_refresh_seconds: row.auto_refresh_seconds,
            default_action_ttl_seconds: row.default_action_ttl_seconds,
            action_polling_enabled: row.action_polling_enabled,
            ping_target_ip: row.ping_target_ip.clone(),
            force_https: row.force_https,
        };

        match shared_settings.write() {
            Ok(mut guard) => *guard = settings_struct,
            Err(_) => return Err(Status::InternalServerError),
        }

        let _ = db::log_audit(
            &mut conn,
            &username,
            "update_settings",
            None,
            Some("Updated server settings"),
        );

        Ok(())
    })
    .await;

    match result {
        Ok(Ok(_)) => Status::Ok,
        _ => Status::InternalServerError,
    }
}
