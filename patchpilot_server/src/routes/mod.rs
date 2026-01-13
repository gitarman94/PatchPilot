use rocket::Route;

pub mod devices;
pub mod actions;
pub mod settings;
pub mod history;
pub mod pages;
pub mod users_groups;
pub mod roles;

pub fn api_routes() -> Vec<Route> {
    let mut routes = Vec::new();
    routes.extend(devices::routes());
    routes.extend(actions::routes());
    routes.extend(settings::routes());
    routes.extend(history::routes());
    routes.extend(users_groups::api_users_groups_routes());
    routes.extend(roles::api_roles_routes());
    routes
}

pub fn page_routes() -> Vec<Route> {
    let mut routes = Vec::new();
    routes.extend(pages::page_routes());
    routes
}

pub fn auth_routes() -> Vec<Route> {
    routes![
        crate::auth::login_page,
        crate::auth::login,
        crate::auth::logout
    ]
}
