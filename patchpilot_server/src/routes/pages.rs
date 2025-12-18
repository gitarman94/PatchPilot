use rocket::fs::NamedFile;

#[get("/")]
pub async fn dashboard() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/dashboard.html").await.ok()
}

#[get("/device_detail.html")]
pub async fn device_detail() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/device_detail.html").await.ok()
}

#[get("/actions.html")]
pub async fn actions_page() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/actions.html").await.ok()
}

#[get("/history.html")]
pub async fn history_page() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/templates/history.html").await.ok()
}

#[get("/favicon.ico")]
pub async fn favicon() -> Option<NamedFile> {
    NamedFile::open("/opt/patchpilot_server/static/favicon.ico").await.ok()
}

#[get("/audit")]
pub async fn audit_page() -> Option<NamedFile> {
    NamedFile::open(Path::new("templates/audit.html")).await.ok()
}