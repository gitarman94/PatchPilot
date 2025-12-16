use chrono::Utc;
use diesel::prelude::*;
use rocket::tokio;
use std::time::Duration;

use crate::db::pool::DbPool;
use crate::models::Action;
use crate::schema::{actions, action_targets};

pub fn spawn_action_ttl_sweeper(pool: DbPool) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;

            let pool = pool.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let mut conn = pool.get().ok()?;

                let expired = actions::table
                    .filter(actions::expires_at.le(Utc::now().naive_utc()))
                    .filter(actions::canceled.eq(false))
                    .load::<Action>(&mut conn)
                    .ok()?;

                for act in expired {
                    let _ = diesel::update(actions::table.filter(actions::id.eq(&act.id)))
                        .set(actions::canceled.eq(true))
                        .execute(&mut conn);

                    let _ = diesel::update(
                        action_targets::table
                            .filter(action_targets::action_id.eq(&act.id))
                            .filter(action_targets::status.eq("pending")),
                    )
                    .set(action_targets::status.eq("expired"))
                    .execute(&mut conn);
                }

                Some(())
            }).await;
        }
    });
}
