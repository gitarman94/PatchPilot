#[launch]
fn rocket() -> _ {
    // Initialize DB + Logger (creates tables + default settings row)
    let pool: DbPool = initialize();

    // Load server settings (ensure DB tables exist first)
    let settings = {
        let mut conn = get_conn(&pool);
        let s = ServerSettings::load(&mut conn)
            .expect("Failed to load server settings from DB");
        Arc::new(RwLock::new(s))
    };

    // System + App State
    let system_state = SystemState::new(pool.clone());

    let app_state = Arc::new(AppState {
        db_pool: pool.clone(),
        system: system_state.clone(),
        pending_devices: Arc::new(RwLock::new(HashMap::new())),
        settings: settings.clone(),
        log_audit: Some(Arc::new(move |conn, actor, action, target, details| {
            if let Err(e) = crate::db::log_audit(conn, actor, action, target, details) {
                log::error!("Audit logging failed: {:?}", e);
            }
        })),
    });

    // Background Tasks
    spawn_action_ttl_task(app_state.clone());
    spawn_pending_cleanup(app_state.clone());

    // Cleanup stale pending devices periodically
    {
        let app_state_clone = app_state.clone();
        rocket::tokio::spawn(async move {
            loop {
                let max_age = {
                    let secs = app_state_clone.settings.read()
                        .map(|s| s.auto_refresh_seconds)
                        .unwrap_or(30);
                    secs.max(30) as u64
                };
                app_state_clone.cleanup_stale_devices(max_age);
                rocket::tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        });
    }

    // Startup Audit Event
    {
        let mut conn = get_conn(&pool);
        let user = AuthUser {
            id: 1,
            username: "admin".to_string(),
            role: UserRole::Admin.as_str().to_string(),
        };
        if let Err(e) = user.audit(&mut conn, "server_started", None) {
            log::error!("Failed to log server start audit: {:?}", e);
        }
    }

    // Refresh system state
    app_state.system.refresh();

    // System Info Logging
    info!(
        "System memory: total {} MB, available {} MB",
        app_state.system.total_memory() / 1024 / 1024,
        app_state.system.available_memory() / 1024 / 1024
    );
    info!("PatchPilot server ready");

    // Rocket Build
    rocket::build()
        .manage(pool)
        .manage(app_state)
        .mount("/api", routes::api_routes())
        .mount("/auth", routes::auth_routes())
        .mount("/users-groups", routes::users_groups::api_users_groups_routes())
        .mount("/roles", routes::roles::api_roles_routes())
        .mount("/history", routes![routes::history::api_history])
        .mount("/audit", routes![routes::history::api_audit])
        .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
        .mount("/", routes::page_routes())
}
