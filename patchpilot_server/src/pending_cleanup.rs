use std::sync::Arc;
use std::time::Duration;

use rocket::{Build, Rocket};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::tokio;

use crate::state::AppState;

pub struct PendingCleanupFairing;

#[rocket::async_trait]
impl Fairing for PendingCleanupFairing {
    fn info(&self) -> Info {
        Info {
            name: "Pending Device Cleanup",
            kind: Kind::Ignite,
        }
    }

    async fn on_ignite(&self, rocket: Rocket<Build>) -> rocket::fairing::Result {
        let state = rocket
            .state::<Arc<AppState>>()
            .expect("AppState not managed")
            .clone();

        tokio::spawn(async move {
            loop {
                let max_age = state
                    .settings
                    .read()
                    .map(|s| s.auto_refresh_seconds.max(30))
                    .unwrap_or(60) as u64;

                state.cleanup_stale_devices(max_age);

                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        Ok(rocket)
    }
}
