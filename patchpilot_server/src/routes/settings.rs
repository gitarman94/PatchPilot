use rocket::{post, State};
use rocket::http::Status;
use rocket::serde::{Deserialize, json::Json};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct BoolSetting {
    pub value: bool,
}

#[derive(Deserialize)]
pub struct IntSetting {
    pub value: u64,
}

#[post("/api/settings/auto_approve", data = "<payload>")]
pub async fn set_auto_approve(
    state: &State<AppState>,
    payload: Json<BoolSetting>,
    user: AuthUser,
) -> Status {
    let username = user.username.clone();
    {
        let mut settings = state.settings.write().unwrap();
        settings.auto_approve_devices = payload.value;
    }

    let mut conn = state.db_pool.get().unwrap(); // if you store pool in state
    log_audit(
        &mut conn,
        &username,
        "set_auto_approve",
        None,
        Some(&format!("auto_approve = {}", payload.value)),
    ).ok();

    Status::Ok
}

#[post("/api/settings/auto_refresh", data = "<payload>")]
pub async fn set_auto_refresh(
    state: &State<AppState>,
    payload: Json<BoolSetting>,
    user: AuthUser,
) -> Status {
    let username = user.username.clone();
    {
        let mut settings = state.settings.write().unwrap();
        settings.auto_refresh_enabled = payload.value;
    }

    let mut conn = state.db_pool.get().unwrap();
    log_audit(
        &mut conn,
        &username,
        "set_auto_refresh",
        None,
        Some(&format!("auto_refresh = {}", payload.value)),
    ).ok();

    Status::Ok
}

#[post("/api/settings/auto_refresh_interval", data = "<payload>")]
pub async fn set_auto_refresh_interval(
    state: &State<AppState>,
    payload: Json<IntSetting>,
    user: AuthUser,
) -> Status {
    let username = user.username.clone();
    {
        let mut settings = state.settings.write().unwrap();
        settings.auto_refresh_seconds = payload.value;
    }

    let mut conn = state.db_pool.get().unwrap();
    log_audit(
        &mut conn,
        &username,
        "set_auto_refresh_interval",
        None,
        Some(&format!("auto_refresh_interval = {}", payload.value)),
    ).ok();

    Status::Ok
}

