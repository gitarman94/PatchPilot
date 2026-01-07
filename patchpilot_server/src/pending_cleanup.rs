use std::sync::Arc;
use std::time::Duration;
use rocket::tokio;

use crate::state::AppState;
use crate::schema::action_targets;
use diesel::prelude::*;

/// Spawns a background task to clean up completed action_targets
pub fn spawn_pending_cleanup(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            // Read cleanup interval from settings
            let interval_secs = {
                let settings = state.settings.read().unwrap();
                settings.auto_refresh_seconds.max(5) // minimum 5 seconds
            };

            tokio::time::sleep(Duration::from_secs(interval_secs as u64)).await;

            // Only proceed if action polling/cleanup is enabled
            let cleanup_enabled = state.settings.read().unwrap().action_polling_enabled;
            if !cleanup_enabled {
                continue;
            }

            // Perform database cleanup
            if let Ok(mut conn) = state.db_pool.get() {
                let cutoff = chrono::Utc::now().naive_utc();

                let deleted_count = diesel::delete(
                    action_targets::table
                        .filter(action_targets::status.eq("completed"))
                        .filter(action_targets::last_update.lt(cutoff)),
                )
                .execute(&mut conn)
                .unwrap_or(0);

                // Log cleanup via audit closure
                if deleted_count > 0 {
                    if let Some(audit) = state.log_audit.as_ref() {
                        let _ = audit(
                            &mut conn,
                            "system",
                            "action.pending_cleanup",
                            None,
                            Some(&format!("Deleted {} completed action_targets", deleted_count)),
                        );
                    }
                }
            }
        }
    });
}
