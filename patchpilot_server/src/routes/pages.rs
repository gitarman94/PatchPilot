use rocket::serde::json::Json;
use rocket_dyn_templates::{Template, context};
use crate::db::DbConn;
use crate::models::{Device, Action, HistoryEntry, User};

#[get("/dashboard")]
pub async fn dashboard(conn: DbConn) -> Template {
    let devices = Device::all(&conn).await.unwrap_or_default();
    Template::render("dashboard", context! {
        devices: devices,
        total_devices: devices.len(),
    })
}

#[get("/devices")]
pub async fn devices_page(conn: DbConn) -> Template {
    let devices = Device::all(&conn).await.unwrap_or_default();
    Template::render("devices", context! {
        devices: devices
    })
}

#[get("/device/<id>")]
pub async fn device_detail(conn: DbConn, id: i32) -> Template {
    let device = Device::find_by_id(&conn, id).await;
    Template::render("device_detail", context! {
        device: device
    })
}

#[get("/actions")]
pub async fn actions_page(conn: DbConn) -> Template {
    let actions = Action::all(&conn).await.unwrap_or_default();
    Template::render("actions", context! {
        actions: actions
    })
}

#[get("/history")]
pub async fn history_page(conn: DbConn) -> Template {
    let history = HistoryEntry::all(&conn).await.unwrap_or_default();
    Template::render("history", context! {
        history: history
    })
}

#[get("/settings")]
pub async fn settings_page(conn: DbConn) -> Template {
    let users = User::all(&conn).await.unwrap_or_default();
    Template::render("settings", context! {
        users: users
    })
}
