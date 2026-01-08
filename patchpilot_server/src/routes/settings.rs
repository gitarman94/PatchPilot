use rocket::{
    get, post,
    State,
    form::{Form, FromForm},
    http::Status,
};
use rocket_dyn_templates::Template;
use diesel::prelude::*;

use crate::state::AppState;
use crate::auth::AuthUser;
use crate::routes::history::log_audit;
use crate::db;
use crate::models::ServerSettings as ModelServerSettings;

#[derive(FromForm)]
pub struct ServerSettingsForm {
    pub max_action_ttl: Option<i64>,
    pub max_pending_age: Option<i64>,
    pub enable_logging: Option<bool>,
    pub default_role: Option<String>,
}


/// VIEW SETTINGS PAGE
#[get("/settings")]
pub async fn view_settings(
    state: &State<AppState>,
    _user: AuthUser,
) -> Result<Template, Status> {
    let pool = state.system.pool.clone();

    let settings: ModelServerSettings = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        db::load_settings(&mut conn).map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    let mut context = std::collections::HashMap::new();
    context.insert("settings", settings);

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

    let pool = state.system.pool.clone();
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
        if let Some(v) = form.max_action_ttl {
            settings.max_action_ttl = v;
        }
        if let Some(v) = form.max_pending_age {
            settings.max_pending_age = v;
        }
        if let Some(v) = form.enable_logging {
            settings.enable_logging = v;
        }
        if let Some(v) = form.default_role {
            settings.default_role = v;
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
        let _ = log_audit(
            &pool,
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
