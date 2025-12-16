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
) -> Status {
    let mut settings = state.settings.write().unwrap();
    settings.auto_approve_devices = payload.value;
    Status::Ok
}

#[post("/api/settings/auto_refresh", data = "<payload>")]
pub async fn set_auto_refresh(
    state: &State<AppState>,
    payload: Json<BoolSetting>,
) -> Status {
    let mut settings = state.settings.write().unwrap();
    settings.auto_refresh_enabled = payload.value;
    Status::Ok
}

#[post("/api/settings/auto_refresh_interval", data = "<payload>")]
pub async fn set_auto_refresh_interval(
    state: &State<AppState>,
    payload: Json<IntSetting>,
) -> Status {
    let mut settings = state.settings.write().unwrap();
    settings.auto_refresh_seconds = payload.value;
    Status::Ok
}
