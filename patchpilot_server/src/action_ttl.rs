use std::sync::Arc;
use std::time::Duration;
use rocket::tokio;

use chrono::Utc;
use diesel::prelude::*;

use crate::state::AppState;
use crate::schema::{actions, action_targets};
use crate::models::ActionTarget;

pub fn spawn_action_ttl_task(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            // Interval from settings
            let interval_secs = {
                let settings = state.settings.read().unwrap();
                settings.auto_refresh_seconds
            };
            tokio::time::sleep(Duration::from_secs(interval_secs as u64)).await;

            // Skip if polling is disabled
            if !state.settings.read().unwrap().action_polling_enabled {
                continue;
            }

            // Get DB connection
            if let Ok(mut conn) = state.db_pool.get() {
                let now = Utc::now().naive_utc();

                // Find expired actions
                let expired_actions = actions::table
                    .filter(actions::expires_at.lt(now))
                    .filter(actions::canceled.eq(false))
                    .load::<String>(&mut conn)
                    .unwrap_or_default();

                for action_id in expired_actions {
                    // Cancel the action
                    let _ = diesel::update(actions::table.filter(actions::id.eq(&action_id)))
                        .set(actions::canceled.eq(true))
                        .execute(&mut conn);

                    // Mark all pending targets as expired
                    let _ = diesel::update(
                        action_targets::table
                            .filter(action_targets::action_id.eq(&action_id))
                            .filter(action_targets::status.eq("pending")),
                    )
                    .set((
                        action_targets::status.eq("expired"),
                        action_targets::last_update.eq(now),
                    ))
                    .execute(&mut conn);

                    // Log audit
                    if let Some(audit) = state.log_audit.as_ref() {
                        let _ = audit(
                            &mut conn,
                            "system",
                            "action.ttl_expired",
                            Some(&action_id),
                            Some("Action automatically canceled due to TTL expiration"),
                        );
                    }
                }
            }
        }
    });
}
