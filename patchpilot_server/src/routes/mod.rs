use rocket::Route;

pub mod devices;
pub mod actions;
pub mod history;
pub mod pages;
pub mod users_groups;
pub mod roles;
pub mod settings;

/// Return API routes mounted under /api
pub fn api_routes() -> Vec<Route> {
    let mut routes: Vec<Route> = Vec::new();

    // devices, actions, and history provide API endpoints and export `routes()`
    routes.extend(devices::routes());
    routes.extend(actions::routes());
    routes.extend(history::routes());

    routes
}

/// Return page (HTML) routes mounted under /
pub fn page_routes() -> Vec<Route> {
    routes![
        pages::dashboard_page,
        pages::devices_page,
        pages::device_detail_page,
        pages::actions_page,
        pages::history_page,
        pages::settings_page
    ]
    .into_iter()
    .collect()
}

/// Authentication endpoints (mounted under /auth)
pub fn auth_routes() -> Vec<Route> {
    routes![
        crate::auth::login_page,
        crate::auth::login,
        crate::auth::logout
    ]
    .into_iter()
    .collect()
}
