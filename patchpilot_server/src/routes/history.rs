use rocket::{get, routes, State};
use rocket::http::Status;
use rocket::serde::json::Json;
use diesel::prelude::*;

use crate::db::DbPool;
use crate::models::{HistoryEntry, AuditLog};
use crate::schema::history_log::dsl::{history_log, created_at as history_created_at};
use crate::schema::audit::dsl::{audit, created_at as audit_created_at};

/// API: GET /api/history
#[get("/")]
pub async fn api_history(pool: &State<DbPool>) -> Result<Json<Vec<HistoryEntry>>, Status> {
    let pool_clone = pool.inner().clone();
    let result: Vec<HistoryEntry> = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<HistoryEntry>, Status> {
        let mut conn = pool_clone.get().map_err(|_| Status::InternalServerError)?;
        let logs = history_log.order(history_created_at.desc()).load::<HistoryEntry>(&mut conn).map_err(|_| Status::InternalServerError)?;
        Ok(logs)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(result))
}

/// API: GET /api/audit (latest 100 entries)
#[get("/audit")]
pub async fn api_audit(pool: &State<DbPool>) -> Result<Json<Vec<AuditLog>>, Status> {
    let pool_clone = pool.inner().clone();
    let result: Vec<AuditLog> = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<AuditLog>, Status> {
        let mut conn = pool_clone.get().map_err(|_| Status::InternalServerError)?;
        let logs = audit.order(audit_created_at.desc()).limit(100).load::<AuditLog>(&mut conn).map_err(|_| Status::InternalServerError)?;
        Ok(logs)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(result))
}

pub fn routes() -> Vec<rocket::Route> {
    routes![api_history, api_audit].into_iter().collect()
}
