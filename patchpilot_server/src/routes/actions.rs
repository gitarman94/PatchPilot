use diesel::prelude::*;
use rocket::{get, post, serde::json::Json, State};
use rocket::http::Status;
use chrono::{Utc, Duration};

use crate::db::{DbPool, log_audit};
use crate::models::{Action, NewAction, ActionTarget};

// Import columns explicitly to avoid ambiguity
use crate::schema::actions::{self, id as action_id_col, created_at, canceled};
use crate::schema::action_targets::{
    self, action_id as at_action_id, device_id as at_device_id, status, last_update, response,
};

/// Placeholder AuthUser; implement FromRequest in your project
pub struct AuthUser {
    pub username: String,
}

impl AuthUser {
    pub fn has_role(&self, _role: &str) -> bool {
        true // Replace with real role check logic
    }
}

/// Submit a new action
#[post("/api/actions", data = "<action>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    action: Json<NewAction>,
    user: AuthUser,
) -> Result<Status, Status> {
    let username = user.username.clone();
    let mut action_data = action.into_inner();
    let pool = pool.inner().clone();

    // Optional sanity check on expires_at
    let min_expiry = Utc::now() + Duration::minutes(5);
    let max_expiry = Utc::now() + Duration::days(7);
    if action_data.expires_at < min_expiry.naive_utc() {
        action_data.expires_at = min_expiry.naive_utc();
    } else if action_data.expires_at > max_expiry.naive_utc() {
        action_data.expires_at = max_expiry.naive_utc();
    }

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        diesel::insert_into(actions::table)
            .values(&action_data)
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(
            &mut conn,
            &username,
            "submit_action",
            Some(&action_data.id),
            Some("Action submitted"),
        )
        .map_err(|_| Status::InternalServerError)?;

        Ok(Status::Created)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

/// List all actions
#[get("/api/actions")]
pub async fn list_actions(pool: &State<DbPool>) -> Result<Json<Vec<Action>>, Status> {
    let pool = pool.inner().clone();

    let result: Vec<Action> = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        actions::table
            .order(created_at.desc())
            .load::<Action>(&mut conn)
            .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(result))
}

/// Cancel an action
#[post("/api/actions/<action_id_param>")]
pub async fn cancel_action(
    pool: &State<DbPool>,
    action_id_param: &str,
    user: AuthUser,
) -> Result<Status, Status> {
    let username = user.username.clone();
    let action_id_str = action_id_param.to_string();
    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        diesel::update(actions::table.filter(action_id_col.eq(&action_id_str)))
            .set(canceled.eq(true))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(
            &mut conn,
            &username,
            "cancel_action",
            Some(&action_id_str),
            Some("Action canceled"),
        )
        .map_err(|_| Status::InternalServerError)?;

        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

/// Report action target result
#[post("/api/actions/<_ignored>/result", data = "<result>")]
pub async fn report_action_result(
    pool: &State<DbPool>,
    _ignored: &str,
    result: Json<ActionTarget>,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let result = result.into_inner();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        diesel::update(
            action_targets::table
                .filter(at_action_id.eq(&result.action_id))
                .filter(at_device_id.eq(&result.device_id)),
        )
        .set((
            status.eq(&result.status),
            last_update.eq(Utc::now().naive_utc()),
            response.eq(&result.response),
        ))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}
