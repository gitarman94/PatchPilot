pub mod devices;
pub mod actions;
pub mod history;
pub mod settings;
pub mod pages;

use rocket::Route;

pub fn api_routes() -> Vec<Route> {
    routes![
        devices::register_device,
        devices::register_or_update_device,
        devices::get_devices,
        devices::get_device_details,
        devices::approve_device,
        devices::heartbeat,

        actions::submit_action,
        actions::report_action_result,
        actions::list_actions,
        actions::cancel_action,

        history::api_history,

        settings::set_auto_approve,
        settings::set_auto_refresh,
        settings::set_auto_refresh_interval,
    ]
}

pub fn page_routes() -> Vec<Route> {
    routes![
        pages::dashboard,
        pages::device_detail,
        pages::actions_page,
        pages::history_page,
        pages::favicon,
    ]
}
