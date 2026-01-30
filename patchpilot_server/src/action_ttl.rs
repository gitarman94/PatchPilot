use std::sync::Arc;
use std::time::Duration;

use rocket::{Build, Rocket};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::tokio;
use chrono::Utc;
use diesel::prelude::*;

use crate::state::AppState;
use crate::schema::{actions, action_targets};

pub struct ActionTtlFairing;

#[rocket::async_trait]
impl Fairing for ActionTtlFairing {
    fn info(&self) -> Info {
        Info {
            name: "Action TTL Cleanup",
            kind: Kind::Ignite,
        }
    }

    async fn on_ignite(&self, rocket: Rocket<Build>) -> rocket::fairing::Result {
        // Expect the managed AppState to be an Arc<AppState>
        let state = rocket
            .state::<Arc<AppState>>()
            .expect("AppState not managed")
            .clone();

        // Spawn the background job that periodically expires actions.
        tokio::spawn(async move {
            loop {
                // Read settings under lock to get interval and whether polling is enabled.
                let (interval_secs, polling_enabled) = {
                    let settings = state.settings.read().unwrap();
                    (settings.auto_refresh_seconds.max(30), settings.action_polling_enabled)
                };

                tokio::time::sleep(Duration::from_secs(interval_secs as u64)).await;

                if !polling_enabled {
                    continue;
                }

                if let Ok(mut conn) = state.db_pool.get() {
                    let now = Utc::now().naive_utc();

                    // Find expired actions that are not already canceled.
                    let expired_action_ids: Vec<i64> = actions::table
                        .select(actions::id)
                        .filter(actions::expires_at.lt(now))
                        .filter(actions::canceled.eq(false))
                        .load::<i64>(&mut conn)
                        .unwrap_or_default();

                    for action_id in expired_action_ids {
                        // Mark action canceled
                        if diesel::update(actions::table.filter(actions::id.eq(action_id)))
                            .set(actions::canceled.eq(true))
                            .execute(&mut conn)
                            .is_err()
                        {
                            continue;
                        }

                        // Mark pending targets as expired and update last_update
                        let _ = diesel::update(
                            action_targets::table
                                .filter(action_targets::action_id.eq(action_id))
                                .filter(action_targets::status.eq("pending")),
                        )
                        .set((
                            action_targets::status.eq("expired"),
                            action_targets::last_update.eq(now),
                        ))
                        .execute(&mut conn);

                        // Audit logging via provided hook in AppState (if present)
                        if let Some(audit_fn) = state.log_audit.as_ref() {
                            let _ = audit_fn(
                                &mut conn,
                                "system",
                                "action.ttl_expired",
                                Some(&action_id.to_string()),
                                Some("Action automatically canceled due to TTL expiration"),
                            );
                        }
                    }
                }
            }
        });

        Ok(rocket)
    }
}
