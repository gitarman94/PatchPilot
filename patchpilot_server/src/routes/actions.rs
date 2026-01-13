use rocket::{get, post, routes, State};
use rocket::http::Status;
use rocket::serde::json::Json;
use diesel::prelude::*;
use chrono::{Utc, Duration};

use crate::db::{DbPool, load_settings, log_audit as db_log_audit, insert_history, update_action_ttl, fetch_action_ttl, NewHistory};
use crate::models::{Action, NewAction, NewActionTarget};
use crate::schema::actions::dsl as actions_dsl;
use crate::schema::action_targets::dsl as action_targets;
use crate::auth::AuthUser;

#[derive(serde::Deserialize)]
pub struct SubmitActionForm {
    pub command: String,
    pub target_device_id: i64,
    pub ttl_seconds: Option<i64>,
}

#[derive(serde::Deserialize)]
pub struct TtlForm {
    pub ttl_seconds: i64,
}

#[post("/api/actions/submit", data = "<form>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    form: Json<SubmitActionForm>,
    user: AuthUser,
) -> Result<Json<serde_json::Value>, Status> {
    let pool_for_db = pool.inner().clone();
    let username = user.username.clone();
    let form = form.into_inner();

    let action_id_res = rocket::tokio::task::spawn_blocking(move || -> Result<i64, Status> {
        let mut conn = pool_for_db.get().map_err(|_| Status::InternalServerError)?;
        let now = Utc::now().naive_utc();

        let settings_row = load_settings(&mut conn).map_err(|_| Status::InternalServerError)?;
        let ttl_to_use = form
            .ttl_seconds
            .map(|t| if t < settings_row.default_action_ttl_seconds { t } else { settings_row.default_action_ttl_seconds })
            .unwrap_or(settings_row.default_action_ttl_seconds);
        let expiry_time = now + Duration::seconds(ttl_to_use);

        let new_action = NewAction {
            action_type: form.command.clone(),
            parameters: None,
            author: Some(username.clone()),
            created_at: now,
            expires_at: expiry_time,
            canceled: false,
        };

        diesel::insert_into(actions_dsl::actions)
            .values(&new_action)
            .execute(&mut conn)
            .map_err(|e| { log::error!("Failed to insert action: {:?}", e); Status::InternalServerError })?;

        let last_id: i64 = actions_dsl::actions
            .select(actions_dsl::id)
            .order(actions_dsl::id.desc())
            .first::<i64>(&mut conn)
            .map_err(|e| { log::error!("Failed to obtain action id: {:?}", e); Status::InternalServerError })?;

        let new_target = NewActionTarget::pending(last_id, form.target_device_id);
        diesel::insert_into(action_targets::action_targets)
            .values(&new_target)
            .execute(&mut conn)
            .map_err(|e| { log::error!("Failed to insert action target: {:?}", e); Status::InternalServerError })?;

        let _ = db_log_audit(
            &mut conn,
            &username,
            &format!("action_submitted:{}", last_id),
            Some(&form.target_device_id.to_string()),
            Some("action submitted"),
        )
        .map_err(|e| { log::warn!("Failed to log audit for action {}: {:?}", last_id, e); Status::InternalServerError })?;

        let history_record = NewHistory {
            action_id: last_id,
            device_name: None,
            actor: Some(&username),
            action_type: "action_submitted",
            details: Some(&form.command),
            created_at: now,
        };
        insert_history(&mut conn, &history_record).map_err(|e| {
            log::warn!("Failed to insert history row for action {}: {:?}", last_id, e);
            Status::InternalServerError
        })?;

        Ok(last_id)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(serde_json::json!({ "action_id": action_id_res, "status": "queued" })))
}

#[get("/api/actions")]
pub async fn list_actions(pool: &State<DbPool>) -> Result<Json<Vec<Action>>, Status> {
    let pool = pool.inner().clone();
    let actions = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<Action>, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let rows = actions_dsl::actions.load::<Action>(&mut conn)
            .map_err(|_| Status::InternalServerError)?;
        Ok(rows)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(actions))
}

/// POST /api/actions/<action_id>/ttl -> extend/update TTL for an action (admin only)
#[post("/api/actions/<action_id>/ttl", data = "<payload>")]
pub async fn extend_action_ttl(
    pool: &State<DbPool>,
    action_id: i64,
    payload: Json<TtlForm>,
    user: AuthUser,
) -> Result<Json<serde_json::Value>, Status> {
    if !user.has_role(crate::auth::RoleName::Admin) {
        return Err(Status::Unauthorized);
    }

    let pool = pool.inner().clone();
    let ttl_val = payload.ttl_seconds;
    let username = user.username.clone();

    rocket::tokio::task::spawn_blocking(move || -> Result<(), Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let settings_row = load_settings(&mut conn).map_err(|_| Status::InternalServerError)?;
        update_action_ttl(&mut conn, action_id, ttl_val, &settings_row)
            .map_err(|_| Status::InternalServerError)?;

        let _ = db_log_audit(
            &mut conn,
            &username,
            "extend_action_ttl",
            Some(&action_id.to_string()),
            Some(&format!("new_ttl={}", ttl_val)),
        )
        .map_err(|e| { log::warn!("Failed to log audit for TTL update {}: {:?}", action_id, e); Status::InternalServerError })?;

        Ok(())
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(serde_json::json!({ "status": "ok", "action_id": action_id })))
}

/// GET /api/actions/<action_id>/ttl -> fetch remaining TTL (seconds)
#[get("/api/actions/<action_id>/ttl")]
pub async fn get_action_ttl(pool: &State<DbPool>, action_id: i64) -> Result<Json<serde_json::Value>, Status> {
    let pool = pool.inner().clone();
    let remaining = rocket::tokio::task::spawn_blocking(move || -> Result<i64, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let secs = fetch_action_ttl(&mut conn, action_id).map_err(|_| Status::InternalServerError)?;
        Ok(secs)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(serde_json::json!({ "action_id": action_id, "ttl_seconds_remaining": remaining })))
}

use rocket::Route;
pub fn routes() -> Vec<Route> {
    routes![submit_action, list_actions, extend_action_ttl, get_action_ttl].into_iter().collect()
}
