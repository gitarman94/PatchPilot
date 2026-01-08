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
use crate::db::{self, ServerSettingsRow};
use crate::models::ServerSettings as ModelServerSettings;

#[derive(FromForm)]
pub struct ServerSettingsForm {
    pub default_action_ttl_seconds: Option<i64>,
    pub default_pending_ttl_seconds: Option<i64>,
    pub logging_enabled: Option<bool>,
    pub default_user_role: Option<String>,
}

#[derive(Serialize)]
struct SettingsContext {
    settings: ModelServerSettings,
}

/// Convert DB row to model
fn row_to_model(row: &ServerSettingsRow) -> ModelServerSettings {
    ModelServerSettings {
        default_action_ttl_seconds: row.default_action_ttl_seconds,
        default_pending_ttl_seconds: row.default_pending_ttl_seconds,
        logging_enabled: row.enable_logging,
        default_user_role: row.default_role.clone(),
    }
}

/// Convert model to DB row
fn model_to_row(model: &ModelServerSettings) -> ServerSettingsRow {
    ServerSettingsRow {
        id: 1,
        force_https: true, // keep existing default
        default_action_ttl_seconds: model.default_action_ttl_seconds,
        default_pending_ttl_seconds: model.default_pending_ttl_seconds,
        enable_logging: model.logging_enabled,
        default_role: model.default_user_role.clone(),
    }
}

/// VIEW SETTINGS PAGE
#[get("/settings")]
pub async fn view_settings(
    state: &State<AppState>,
    _user: AuthUser,
) -> Result<Template, Status> {
    let pool = state.pool.clone();

    let settings_model: ModelServerSettings = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let row = db::load_settings(&mut conn).map_err(|_| Status::InternalServerError)?;
        Ok(row_to_model(&row))
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
    let pool = state.pool.clone();
    let shared_settings = state.settings.clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };

        // Load existing settings
        let mut row = match db::load_settings(&mut conn) {
            Ok(s) => s,
            Err(_) => return,
        };

        // Apply updates
        if let Some(v) = form.default_action_ttl_seconds {
            row.default_action_ttl_seconds = v;
        }
        if let Some(v) = form.default_pending_ttl_seconds {
            row.default_pending_ttl_seconds = v;
        }
        if let Some(v) = form.logging_enabled {
            row.enable_logging = v;
        }
        if let Some(v) = form.default_user_role {
            row.default_role = v;
        }

        // Persist to DB
        if db::save_settings(&mut conn, &row).is_err() {
            return;
        }

        // Update in-memory shared settings
        if let Ok(mut guard) = shared_settings.write() {
            *guard = row_to_model(&row);
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
