use rocket::Route;

pub mod devices;
pub mod actions;
pub mod settings;
pub mod history;
pub mod pages;
pub mod users_groups;
pub mod roles;

/// API routes (JSON API)
pub fn api_routes() -> Vec<Route> {
    let mut routes = Vec::new();
    routes.extend(devices::api_routes());
    routes.extend(actions::api_routes());
    routes.extend(settings::api_routes());
    routes.extend(history::api_routes());
    routes.extend(users_groups::api_users_groups_routes());
    routes.extend(roles::api_roles_routes());
    routes
}

/// Page routes (HTML pages)
pub fn page_routes() -> Vec<Route> {
    let mut routes = Vec::new();
    routes.extend(pages::page_routes());
    routes
}

/// Authentication routes (login/logout)
pub fn auth_routes() -> Vec<Route> {
    routes![
        crate::auth::login_page,
        crate::auth::login,
        crate::auth::logout
    ]
}
