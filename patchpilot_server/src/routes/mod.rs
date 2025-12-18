use rocket::Route;

mod devices;
mod actions;
mod settings;
mod history;
mod pages;
mod auth;
mod users_groups;
mod roles;

/// API routes
pub fn api_routes() -> Vec<Route> {
    routes![
        // Devices
        devices::get_devices,
        devices::get_device_details,
        devices::approve_device,
        devices::register_device_route,
        devices::register_or_update_device,
        devices::heartbeat_route,

        // Actions
        actions::submit_action,
        actions::report_action_result,
        actions::list_actions,
        actions::cancel_action,

        // History
        history::api_history,

        // Settings
        settings::set_auto_approve,
        settings::set_auto_refresh,
        settings::set_auto_refresh_interval
    ]
}

/// Page routes
pub fn page_routes() -> Vec<Route> {
    routes![
        pages::dashboard,
        pages::device_detail,
        pages::actions_page,
        pages::history_page,
        pages::audit_page,
        pages::favicon,
    ]
}

/// Auth routes (login/logout)
pub fn auth_routes() -> Vec<Route> {
    routes![
        auth::login_page,
        auth::login,
        auth::logout
    ]
}

/// Users & Groups routes
pub fn users_groups_routes() -> Vec<Route> {
    routes![
        users_groups::list_users_groups,
        users_groups::add_user,
        users_groups::delete_user,
        users_groups::add_group,
        users_groups::delete_group
    ]
}

/// Roles routes
pub fn roles_routes() -> Vec<Route> {
    routes![
        roles::list_roles,
        roles::add_role,
        roles::delete_role
    ]
}
