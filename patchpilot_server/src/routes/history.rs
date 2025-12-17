use rocket::{get, State, http::Status};
use rocket::serde::json::Json;
use diesel::prelude::*;
use crate::db::pool::DbPool;
use crate::models::HistoryLog;
use crate::schema::history_log::dsl::*;

/// API: GET /api/history
#[get("/history")]
pub async fn api_history(
    pool: &State<DbPool>,
) -> Result<Json<Vec<HistoryLog>>, Status> {
    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        history_log
            .order(created_at.desc())
            .load::<HistoryLog>(&mut conn) // type annotation fixes type inference
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}
