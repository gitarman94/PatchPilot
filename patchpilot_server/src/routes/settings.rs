use rocket::{post, State};
use rocket::http::Status;
use rocket::serde::{Deserialize, json::Json};

use crate::state::AppState;
use crate::auth::AuthUser;
use crate::routes::history::log_audit;
use crate::db::DbPool;

#[derive(Deserialize)]
pub struct BoolSetting {
    pub value: bool,
}

#[derive(Deserialize)]
pub struct IntSetting {
    pub value: i64,
}

#[post("/api/settings/auto_approve", data = "<payload>")]
pub async fn set_auto_approve(
    state: &State<AppState>,
    payload: Json<BoolSetting>,
    user: AuthUser,
) -> Status {
    let username = user.username.clone();
    let value = payload.value;

    // Update in-memory settings
    {
        let mut settings = state.settings.write().unwrap();
        settings.auto_approve_devices = value;
    }

    // Log audit asynchronously
    let pool = state.system.db_pool.clone();
    rocket::tokio::task::spawn_blocking(move || {
        if let Ok(mut conn) = pool.get() {
            log_audit(
                &mut conn,
                &username,
                "set_auto_approve",
                None,
                Some(&format!("auto_approve = {}", value)),
            ).ok();
        }
    }).await.ok();

    Status::Ok
}

#[post("/api/settings/auto_refresh", data = "<payload>")]
pub async fn set_auto_refresh(
    state: &State<AppState>,
    payload: Json<BoolSetting>,
    user: AuthUser,
) -> Status {
    let username = user.username.clone();
    let value = payload.value;

    {
        let mut settings = state.settings.write().unwrap();
        settings.auto_refresh_enabled = value;
    }

    let pool = state.system.db_pool.clone();
    rocket::tokio::task::spawn_blocking(move || {
        if let Ok(mut conn) = pool.get() {
            log_audit(
                &mut conn,
                &username,
                "set_auto_refresh",
                None,
                Some(&format!("auto_refresh = {}", value)),
            ).ok();
        }
    }).await.ok();

    Status::Ok
}

#[post("/api/settings/auto_refresh_interval", data = "<payload>")]
pub async fn set_auto_refresh_interval(
    state: &State<AppState>,
    payload: Json<IntSetting>,
    user: AuthUser,
) -> Status {
    let username = user.username.clone();
    let value = payload.value;

    {
        let mut settings = state.settings.write().unwrap();
        settings.auto_refresh_seconds = value;
    }

    let pool = state.system.db_pool.clone();
    rocket::tokio::task::spawn_blocking(move || {
        if let Ok(mut conn) = pool.get() {
            log_audit(
                &mut conn,
                &username,
                "set_auto_refresh_interval",
                None,
                Some(&format!("auto_refresh_interval = {}", value)),
            ).ok();
        }
    }).await.ok();

    Status::Ok
}
