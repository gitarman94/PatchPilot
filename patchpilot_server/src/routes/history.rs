use rocket::{get, State, http::Status};
use rocket::serde::json::Json;
use diesel::prelude::*;

use crate::db::{DbPool, log_audit as db_log_audit};
use crate::models::{HistoryLog, AuditLog};
use crate::schema::history_log::dsl::{history_log, created_at as history_created_at};
use crate::schema::audit::dsl::{audit, created_at as audit_created_at};

/// API: GET /api/history
#[get("/api/history")]
pub async fn api_history(pool: &State<DbPool>) -> Result<Json<Vec<HistoryLog>>, Status> {
    let pool = pool.inner().clone();

    let result: Vec<HistoryLog> = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<HistoryLog>, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let logs = history_log
            .order(history_created_at.desc())
            .load::<HistoryLog>(&mut conn)
            .map_err(|_| Status::InternalServerError)?;
        Ok(logs)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
    ?;

    Ok(Json(result))
}

/// API: GET /api/audit (latest 100 entries)
#[get("/api/audit")]
pub async fn api_audit(pool: &State<DbPool>) -> Result<Json<Vec<AuditLog>>, Status> {
    let pool = pool.inner().clone();

    let result: Vec<AuditLog> = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<AuditLog>, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let logs = audit
            .order(audit_created_at.desc())
            .limit(100)
            .load::<AuditLog>(&mut conn)
            .map_err(|_| Status::InternalServerError)?;
        Ok(logs)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
    ?;

    Ok(Json(result))
}

/// Async helper: log an audit action
pub async fn log_audit(
    pool: &DbPool,
    actor_val: &str,
    action_type_val: &str,
    target_val: Option<&str>,
    details_val: Option<&str>,
) -> Result<(), Status> {
    let pool = pool.clone();
    let actor_val = actor_val.to_string();
    let action_type_val = action_type_val.to_string();
    let target_val = target_val.map(|s| s.to_string());
    let details_val = details_val.map(|s| s.to_string());

    rocket::tokio::task::spawn_blocking(move || -> Result<(), Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        db_log_audit(
            &mut conn,
            &actor_val,
            &action_type_val,
            target_val.as_deref(),
            details_val.as_deref(),
        )
        .map_err(|_| Status::InternalServerError)?;
        Ok(())
    })
    .await
    .map_err(|_| Status::InternalServerError)?
    ?;

    Ok(())
}
