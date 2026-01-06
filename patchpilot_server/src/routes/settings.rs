use rocket::{get, post, State, form::{Form}};
use rocket::http::Status;
use diesel::prelude::*;
use crate::auth::AuthUser;
use crate::db::{DbPool};
use crate::routes::history::log_audit;
use crate::schema::server_settings;

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
    pool: &State<DbPool>,
    _user: AuthUser,
) -> Result<rocket_dyn_templates::Template, Status> {
    let pool = pool.inner().clone();

    let settings = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        crate::db::load_settings(&mut conn).map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    let mut context = std::collections::HashMap::new();
    context.insert("settings", settings);
    Ok(rocket_dyn_templates::Template::render("settings", &context))
}

#[post("/settings/update", data = "<form>")]
pub async fn update_settings(
    pool: &State<DbPool>,
    form: Form<ServerSettingsForm>,
    user: AuthUser,
) -> Result<Status, Status> {
    let username = user.username.clone();
    let form = form.into_inner();
    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return Err(Status::InternalServerError),
        };

        let current = match crate::db::load_settings(&mut conn) {
            Ok(s) => s,
            Err(_) => return Err(Status::InternalServerError),
        };

        let mut details = Vec::new();
        let mut updated_settings = current.clone();

        if let Some(v) = form.auto_approve_devices {
            details.push(format!("auto_approve_devices: {} -> {}", current.auto_approve_devices, v));
            updated_settings.auto_approve_devices = v;
            let _ = diesel::update(server_settings::table)
                .set(server_settings::auto_approve_devices.eq(v))
                .execute(&mut conn);
        }

        if let Some(v) = form.auto_refresh_enabled {
            details.push(format!("auto_refresh_enabled: {} -> {}", current.auto_refresh_enabled, v));
            updated_settings.auto_refresh_enabled = v;
            let _ = diesel::update(server_settings::table)
                .set(server_settings::auto_refresh_enabled.eq(v))
                .execute(&mut conn);
        }

        if let Some(v) = form.auto_refresh_seconds {
            details.push(format!("auto_refresh_seconds: {} -> {}", current.auto_refresh_seconds, v));
            updated_settings.auto_refresh_seconds = v;
            let _ = diesel::update(server_settings::table)
                .set(server_settings::auto_refresh_seconds.eq(v))
                .execute(&mut conn);
        }

        if let Some(v) = form.default_action_ttl_seconds {
            details.push(format!("default_action_ttl_seconds: {} -> {}", current.default_action_ttl_seconds, v));
            updated_settings.default_action_ttl_seconds = v;
        }

        if let Some(v) = form.action_polling_enabled {
            details.push(format!("action_polling_enabled: {} -> {}", current.action_polling_enabled, v));
            updated_settings.action_polling_enabled = v;
            let _ = diesel::update(server_settings::table)
                .set(server_settings::action_polling_enabled.eq(v))
                .execute(&mut conn);
        }

        if let Some(v) = form.ping_target_ip.clone() {
            details.push(format!("ping_target_ip: {} -> {}", current.ping_target_ip, v));
            updated_settings.ping_target_ip = v.clone();
            let _ = diesel::update(server_settings::table)
                .set(server_settings::ping_target_ip.eq(v))
                .execute(&mut conn);
        }

        let _ = crate::db::save_settings(&mut conn, &updated_settings);

        let detail_str = details.join("; ");
        let _ = log_audit(
            &mut conn,
            &username,
            "settings.update",
            None,
            Some(&detail_str),
        );

        Ok::<(), Status>(())
    })
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}
