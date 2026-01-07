use rocket::Route;

pub mod devices;
pub mod actions;
pub mod settings;
pub mod history;
pub mod pages;
pub mod users_groups;
pub mod roles;

/// API routes
pub fn api_routes() -> Vec<Route> {
    routes![
        // Devices
        devices::get_devices,
        devices::get_device_details,
        devices::approve_device,
        devices::register_or_update_device,
        devices::heartbeat,

        // Actions
        actions::submit_action,
        actions::report_action_result,
        actions::list_actions,
        actions::list_action_targets,
        actions::update_action_ttl,
        actions::pending_cleanup,
        actions::cancel_action,

        // History
        history::api_history,

        // Settings
        settings::view_settings,
        settings::update_settings,

        // Users & Groups
        users_groups::list_users_groups,
        users_groups::add_user,
        users_groups::delete_user,
        users_groups::add_group,
        users_groups::delete_group,

        // Roles
        roles::list_roles,
        roles::add_role,
        roles::delete_role,
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
        crate::auth::login_page,
        crate::auth::login,
        crate::auth::logout,
    ]
}
