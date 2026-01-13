use rocket::{get, State};
use rocket_dyn_templates::{Template, context};

use crate::db::DbPool;
use crate::models::{Device, Action, HistoryEntry, User};


#[get("/dashboard")]
pub async fn dashboard(pool: &State<DbPool>) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => {
            return Template::render("dashboard", context! { devices: Vec::<Device>::new(), total_devices: 0 });
        }
    };

    let devices = Device::all(&mut conn).unwrap_or_default();
    let total_devices = devices.len();

    Template::render("dashboard", context! {
        devices: devices,
        total_devices: total_devices,
    })
}

#[get("/devices")]
pub async fn devices_page(pool: &State<DbPool>) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("devices", context! { devices: Vec::<Device>::new() }),
    };

    let devices = Device::all(&mut conn).unwrap_or_default();

    Template::render("devices", context! { devices: devices })
}

#[get("/device/<id>")]
pub async fn device_detail(pool: &State<DbPool>, id: i64) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("device_detail", context! { device: Option::<Device>::None }),
    };

    let device = Device::find_by_device_id(&mut conn, id).ok();

    Template::render("device_detail", context! { device: device })
}

#[get("/actions")]
pub async fn actions_page(pool: &State<DbPool>) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("actions", context! { actions: Vec::<Action>::new() }),
    };

    let actions = Action::all(&mut conn).unwrap_or_default();

    Template::render("actions", context! { actions: actions })
}

#[get("/history")]
pub async fn history_page(pool: &State<DbPool>) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("history", context! { history: Vec::<HistoryEntry>::new() }),
    };

    let history = HistoryEntry::all(&mut conn).unwrap_or_default();

    Template::render("history", context! { history: history })
}

#[get("/settings")]
pub async fn settings_page(pool: &State<DbPool>) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("settings", context! { users: Vec::<User>::new() }),
    };

    let users = User::all(&mut conn).unwrap_or_default();

    Template::render("settings", context! { users: users })
}
