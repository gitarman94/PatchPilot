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
use crate::models::ServerSettings as ModelServerSettings;

#[derive(FromForm)]
pub struct ServerSettingsForm {
    pub default_action_ttl_seconds: Option<i64>,
    pub max_pending_actions_seconds: Option<i64>,
    pub logging_enabled: Option<bool>,
    pub default_user_role: Option<String>,
}

#[derive(Serialize)]
struct SettingsContext {
    settings: ModelServerSettings,
}

/// VIEW SETTINGS PAGE
#[get("/settings")]
pub async fn view_settings(
    state: &State<AppState>,
    _user: AuthUser,
) -> Result<Template, Status> {
    let pool = state.pool.clone();

    let settings: ModelServerSettings = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        db::load_settings(&mut conn).map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    let context = SettingsContext {
        settings: settings.clone(),
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
        let mut settings = match db::load_settings(&mut conn) {
            Ok(s) => s,
            Err(_) => return,
        };

        // Apply updates
        if let Some(v) = form.default_action_ttl_seconds {
            settings.default_action_ttl_seconds = v;
        }
        if let Some(v) = form.max_pending_actions_seconds {
            settings.max_pending_actions_seconds = v;
        }
        if let Some(v) = form.logging_enabled {
            settings.logging_enabled = v;
        }
        if let Some(v) = form.default_user_role {
            settings.default_user_role = v;
        }

        // Persist to DB
        if db::save_settings(&mut conn, &settings).is_err() {
            return;
        }

        // Update in-memory shared settings
        if let Ok(mut guard) = shared_settings.write() {
            *guard = settings.clone();
        }

        // Audit log
        use futures::executor::block_on;
        let _ = block_on(log_audit(&pool, &username, "update_settings", None, Some("Updated server settings")));
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
