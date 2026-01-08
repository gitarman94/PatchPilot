// src/action_ttl.rs
use std::sync::Arc;
use std::time::Duration;
use rocket::tokio;
use chrono::Utc;
use diesel::prelude::*;

use crate::state::AppState;
use crate::schema::{actions, action_targets};

/// Spawn a background task that automatically cancels expired actions
pub fn spawn_action_ttl_task(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            // Interval from settings
            let (interval_secs, polling_enabled) = {
                let settings = state.settings.read().unwrap();
                (settings.auto_refresh_seconds, settings.action_polling_enabled)
            };

            tokio::time::sleep(Duration::from_secs(interval_secs as u64)).await;

            // Skip if polling is disabled
            if !polling_enabled {
                continue;
            }

            // Get DB connection
            if let Ok(mut conn) = state.db_pool.get() {
                let now = Utc::now().naive_utc();

                // Select only action IDs that have expired and are not canceled
                let expired_action_ids: Vec<i64> = actions::table
                    .select(actions::id)
                    .filter(actions::expires_at.lt(now))
                    .filter(actions::canceled.eq(false))
                    .load::<i64>(&mut conn)
                    .unwrap_or_default();

                for action_id in expired_action_ids {
                    // Mark action as canceled
                    if let Err(e) = diesel::update(actions::table.filter(actions::id.eq(action_id)))
                        .set(actions::canceled.eq(true))
                        .execute(&mut conn)
                    {
                        eprintln!("Failed to cancel action {}: {:?}", action_id, e);
                        continue;
                    }

                    // Mark pending targets as expired
                    if let Err(e) = diesel::update(
                        action_targets::table
                            .filter(action_targets::action_id.eq(action_id))
                            .filter(action_targets::status.eq("pending")),
                    )
                    .set((
                        action_targets::status.eq("expired"),
                        action_targets::last_update.eq(now),
                    ))
                    .execute(&mut conn)
                    {
                        eprintln!("Failed to update action_targets for {}: {:?}", action_id, e);
                    }

                    // Audit log - our stored closure returns a Diesel QueryResult<()>
                    if let Some(audit_fn) = state.log_audit.as_ref() {
                        if let Err(e) = audit_fn(
                            &mut conn,
                            "system",
                            "action.ttl_expired",
                            Some(&action_id.to_string()),
                            Some("Action automatically canceled due to TTL expiration"),
                        ) {
                            eprintln!("Failed to log audit for action {}: {:?}", action_id, e);
                        }
                    }
                } // for expired_action_ids
            } // if let Ok(mut conn)
        } // loop
    });
}
