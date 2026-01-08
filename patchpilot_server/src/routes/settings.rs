use rocket::{get, post, State, form::{Form, FromForm}};
use rocket::http::Status;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::state::AppState;
use crate::auth::AuthUser;
use crate::routes::history::log_audit;
use crate::schema::server_settings;
use crate::db;
use crate::models::ServerSettings as ModelServerSettings;

#[derive(FromForm)]
pub struct ServerSettingsForm {
    pub max_action_ttl: Option<i64>,
    pub max_pending_age: Option<i64>,
    pub enable_logging: Option<bool>,
    pub default_role: Option<String>,
}

/// View current server settings
#[get("/settings")]
pub async fn view_settings(
    state: &State<AppState>,
    _user: AuthUser,
) -> Result<rocket_dyn_templates::Template, Status> {
    let pool = state.system.pool.clone();

    let settings: ModelServerSettings = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        db::load_settings(&mut conn).map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    let mut context = std::collections::HashMap::new();
    context.insert("settings", settings);
    Ok(rocket_dyn_templates::Template::render("settings", &context))
}

/// Update server settings
#[post("/settings/update", data = "<form>")]
pub async fn update_settings(
    state: &State<AppState>,
    form: Form<ServerSettingsForm>,
    user: AuthUser,
) -> Status {
    let username = user.username.clone();
    let form = form.into_inner();
    let pool = state.system.pool.clone();
    let settings_arc = state.settings.clone();

    rocket::tokio::task::spawn_blocking(move || {
        // Get DB connection
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };

        // Load current settings
        let mut settings: ModelServerSettings = match db::load_settings(&mut conn) {
            Ok(s) => s,
            Err(_) => return,
        };

        // Apply changes from form
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

        // Save updated settings back to DB
        let _ = db::save_settings(&mut conn, &settings);

        // Update shared in-memory settings
        if let Ok(mut shared) = settings_arc.write() {
            *shared = settings.clone();
        }

        // Log audit entry
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

/// Helper: update max_action_ttl
pub fn set_max_action_ttl(
    conn: &mut SqliteConnection,
    value: i64,
) -> diesel::QueryResult<usize> {
    diesel::update(server_settings::table)
        .set(server_settings::max_action_ttl.eq(value))
        .execute(conn)
}

/// Helper: update max_pending_age
pub fn set_max_pending_age(
    conn: &mut SqliteConnection,
    value: i64,
) -> diesel::QueryResult<usize> {
    diesel::update(server_settings::table)
        .set(server_settings::max_pending_age.eq(value))
        .execute(conn)
}

/// Helper: update enable_logging
pub fn set_enable_logging(
    conn: &mut SqliteConnection,
    value: bool,
) -> diesel::QueryResult<usize> {
    diesel::update(server_settings::table)
        .set(server_settings::enable_logging.eq(value))
        .execute(conn)
}

/// Helper: update default_role
pub fn set_default_role(
    conn: &mut SqliteConnection,
    value: &str,
) -> diesel::QueryResult<usize> {
    diesel::update(server_settings::table)
        .set(server_settings::default_role.eq(value))
        .execute(conn)
}

/// Mount all settings-related routes
pub fn configure_routes(rocket: rocket::Rocket<rocket::Build>) -> rocket::Rocket<rocket::Build> {
    rocket.mount("/", routes![view_settings, update_settings])
}
