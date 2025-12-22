use rocket::{get, post, State, form::Form};
use rocket::http::Status;
use crate::state::AppState;
use crate::auth::AuthUser;
use crate::routes::history::log_audit;
use crate::db::DbPool;

/// Struct representing form submission for server settings
#[derive(FromForm)]
pub struct ServerSettingsForm {
    pub auto_approve_devices: Option<bool>,
    pub auto_refresh_enabled: Option<bool>,
    pub auto_refresh_seconds: Option<i64>,
    pub default_action_ttl_seconds: Option<i64>,
    pub action_polling_enabled: Option<bool>,
    pub ping_target_ip: Option<String>,
}

/// Render the settings page
#[get("/settings")]
pub async fn view_settings(state: &State<AppState>, _user: AuthUser) -> Result<rocket_dyn_templates::Template, Status> {
    let pool = state.system.db_pool.clone();
    let settings = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        crate::db::load_settings(&mut conn).map_err(|_| Status::InternalServerError)
    }).await.map_err(|_| Status::InternalServerError)??;

    let mut context = std::collections::HashMap::new();
    context.insert("settings", settings);

    Ok(rocket_dyn_templates::Template::render("settings", &context))
}

/// Update server settings via HTML form
#[post("/settings/update", data = "<form>")]
pub async fn update_settings(
    state: &State<AppState>,
    form: Form<ServerSettingsForm>,
    user: AuthUser,
) -> Status {
    let username = user.username.clone();
    let form = form.into_inner();

    let pool = state.system.db_pool.clone();
    rocket::tokio::task::spawn_blocking(move || {
        if let Ok(mut conn) = pool.get() {
            // Load current settings
            let mut settings = crate::db::load_settings(&mut conn).unwrap_or_default();

            // Update fields from form
            if let Some(v) = form.auto_approve_devices { settings.auto_approve_devices = v; }
            if let Some(v) = form.auto_refresh_enabled { settings.auto_refresh_enabled = v; }
            if let Some(v) = form.auto_refresh_seconds { settings.auto_refresh_seconds = v; }
            if let Some(v) = form.default_action_ttl_seconds { settings.default_action_ttl_seconds = v; }
            if let Some(v) = form.action_polling_enabled { settings.action_polling_enabled = v; }
            if let Some(v) = form.ping_target_ip { settings.ping_target_ip = v; }

            // Save to DB
            let _ = crate::db::save_settings(&mut conn, &settings);

            // Update in-memory copy
            {
                let mut shared_settings = state.settings.write().unwrap();
                *shared_settings = settings.clone();
            }

            // Audit logging
            let _ = log_audit(
                &mut conn,
                &username,
                "update_settings",
                None,
                Some(&format!("Updated server settings")),
            );
        }
    }).await.ok();

    Status::Ok
}
