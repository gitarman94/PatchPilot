use rocket::{get, State};
use rocket::response::Redirect;
use rocket_dyn_templates::{Template, context};
use crate::db::DbPool;
use crate::models::{Device, Action as ActionModel, HistoryEntry, User};

/// Root -> redirect to dashboard
#[get("/")]
pub async fn index() -> Redirect {
    Redirect::to(uri!(dashboard_page))
}

/// Dashboard page — provides aggregates used by the dashboard.hbs template
#[get("/dashboard")]
pub async fn dashboard_page(pool: &State<DbPool>) -> Template {
    // try get connection from pool; on error render an empty dashboard context
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => {
            return Template::render(
                "dashboard",
                context! {
                    devices: Vec::<Device>::new(),
                    total_devices: 0usize,
                    approved_devices: 0usize,
                    pending_devices: 0usize,
                    total_actions: 0usize,
                },
            );
        }
    };

    // load devices
    let devices = Device::all(&mut conn).unwrap_or_default();
    let total_devices = devices.len();
    // devices.approved is a bool in the model; count defensively
    let approved_devices = devices.iter().filter(|d| d.approved).count();
    let pending_devices = total_devices.saturating_sub(approved_devices);

    // load actions (safe fallback)
    let actions = ActionModel::all(&mut conn).unwrap_or_default();
    let total_actions = actions.len();

    Template::render(
        "dashboard",
        context! {
            devices: devices,
            total_devices: total_devices,
            approved_devices: approved_devices,
            pending_devices: pending_devices,
            total_actions: total_actions,
        },
    )
}

/// Devices list page
#[get("/devices_page")]
pub async fn devices_page(pool: &State<DbPool>) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("devices", context! { devices: Vec::<Device>::new() }),
    };
    let devices = Device::all(&mut conn).unwrap_or_default();
    Template::render("devices", context! { devices: devices })
}

/// Device detail page — accepts an `id` path parameter
#[get("/device_detail/<id>")]
pub async fn device_detail_page(pool: &State<DbPool>, id: i64) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("device_detail", context! { device: Option::<Device>::None }),
    };
    let device = Device::find_by_device_id(&mut conn, id).ok();
    Template::render("device_detail", context! { device: device })
}

/// Actions page
#[get("/actions_page")]
pub async fn actions_page(pool: &State<DbPool>) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("actions", context! { actions: Vec::<ActionModel>::new() }),
    };
    let actions = ActionModel::all(&mut conn).unwrap_or_default();
    Template::render("actions", context! { actions: actions })
}

/// History page
#[get("/history_page")]
pub async fn history_page(pool: &State<DbPool>) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("history", context! { history: Vec::<HistoryEntry>::new() }),
    };
    let history = HistoryEntry::all(&mut conn).unwrap_or_default();
    Template::render("history", context! { history: history })
}

/// Settings page
#[get("/settings_page")]
pub async fn settings_page(pool: &State<DbPool>) -> Template {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("settings", context! { users: Vec::<User>::new() }),
    };
    let users = User::all(&mut conn).unwrap_or_default();
    Template::render("settings", context! { users: users })
}
