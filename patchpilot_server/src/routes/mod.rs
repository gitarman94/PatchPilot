use rocket::Route;

mod devices;
mod actions;
mod settings;
mod history;
mod pages;

pub fn api_routes() -> Vec<Route> {
    routes![
        // Devices
        devices::get_devices,
        devices::get_device_details,
        devices::approve_device,
        devices::register_device,
        devices::register_or_update_device,
        devices::heartbeat,

        // Actions
        actions::submit_action,
        actions::report_action_result, // <- matches pub fn in actions.rs
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

