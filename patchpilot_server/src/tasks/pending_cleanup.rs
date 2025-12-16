use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use rocket::tokio;

use crate::state::AppState;

pub fn spawn_pending_cleanup(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut last_seen: HashMap<String, Instant> = HashMap::new();

        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let now = Instant::now();

            let mut pending = state.pending_devices.write().unwrap();
            for id in pending.keys() {
                last_seen.insert(id.clone(), now);
            }

            pending.retain(|id, _| {
                last_seen
                    .get(id)
                    .map(|t| now.duration_since(*t) < Duration::from_secs(15))
                    .unwrap_or(false)
            });

            last_seen.retain(|id, _| pending.contains_key(id));
        }
    });
}
