use rocket::{get, post, State, form::{Form, FromForm}};
use rocket::http::Status;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::state::AppState;
use crate::auth::AuthUser;
use crate::routes::history::log_audit;
use crate::schema::server_settings;
use crate::db;
use crate::models::ServerSettings as ModelSettings;

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

#[get("/settings")]
pub async fn view_settings(
    state: &State<AppState>,
    _user: AuthUser,
) -> Result<rocket_dyn_templates::Template, Status> {
    let pool = state.system.db_pool.clone();

    let settings: ModelSettings = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let s: crate::settings::ServerSettings = db::load_settings(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        Ok(ModelSettings {
            id: 1,
            auto_approve_devices: s.auto_approve_devices,
            auto_refresh_enabled: s.auto_refresh_enabled,
            auto_refresh_seconds: s.auto_refresh_seconds,
            default_action_ttl_seconds: s.default_action_ttl_seconds,
            action_polling_enabled: s.action_polling_enabled,
            ping_target_ip: s.ping_target_ip,
        })
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    let mut context = std::collections::HashMap::new();
    context.insert("settings", settings);
    Ok(rocket_dyn_templates::Template::render("settings", &context))
}

#[post("/settings/update", data = "<form>")]
pub async fn update_settings(
    state: &State<AppState>,
    form: Form<ServerSettingsForm>,
    user: AuthUser,
) -> Status {
    let username = user.username.clone();
    let form = form.into_inner();
    let pool = state.system.db_pool.clone();
    let settings_arc = state.settings.clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut settings: crate::settings::ServerSettings = match db::load_settings(&mut conn) {
            Ok(s) => s,
            Err(_) => return,
        };

        if let Some(v) = form.auto_approve_devices {
            let _ = set_auto_approve(&mut conn, v);
            settings.auto_approve_devices = v;
        }
        if let Some(v) = form.auto_refresh_enabled {
            let _ = set_auto_refresh(&mut conn, v);
            settings.auto_refresh_enabled = v;
        }
        if let Some(v) = form.auto_refresh_seconds {
            let _ = set_auto_refresh_interval(&mut conn, v);
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

        let _ = db::save_settings(&mut conn, &settings);
        if let Ok(mut shared) = settings_arc.write() {
            *shared = settings.clone();
        }

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

/* Direct DB setters — actively used */
pub fn set_auto_approve(
    conn: &mut SqliteConnection,
    value: bool,
) -> diesel::QueryResult<usize> {
    diesel::update(server_settings::table)
        .set(server_settings::auto_approve_devices.eq(value))
        .execute(conn)
}

pub fn set_auto_refresh(
    conn: &mut SqliteConnection,
    value: bool,
) -> diesel::QueryResult<usize> {
    diesel::update(server_settings::table)
        .set(server_settings::auto_refresh_enabled.eq(value))
        .execute(conn)
}

pub fn set_auto_refresh_interval(
    conn: &mut SqliteConnection,
    value: i64,
) -> diesel::QueryResult<usize> {
    diesel::update(server_settings::table)
        .set(server_settings::auto_refresh_seconds.eq(value))
        .execute(conn)
}
