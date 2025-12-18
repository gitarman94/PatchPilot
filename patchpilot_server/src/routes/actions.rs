use diesel::prelude::*;
use rocket::{get, post, serde::json::Json, State};
use rocket::http::Status;
use chrono::Utc;

use crate::db::pool::DbPool;
use crate::models::{Action, NewAction, ActionTarget};

// Explicitly import columns to avoid ambiguity
use crate::schema::actions::{self, id as action_id_col, created_at, canceled};
use crate::schema::action_targets::{self, action_id as at_action_id, device_id as at_device_id, status, last_update, response};

/// Submit a new action
#[post("/api/actions", data = "<action>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    action: Json<NewAction>,
    user: AuthUser,
) -> Result<Status, Status> {
    let username = user.username.clone();
    let action_data = action.into_inner();
    let pool = pool.inner().clone();

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
        ).map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
    .map(|_| Status::Created)
}

/// List all actions
#[get("/api/actions")]
pub async fn list_actions(pool: &State<DbPool>) -> Result<Json<Vec<Action>>, Status> {
    let pool = pool.inner().clone();

    let result = rocket::tokio::task::spawn_blocking(move || {
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

/// Cancel an action by ID
#[post("/api/actions/<action_id>")]
pub async fn cancel_action(
    pool: &State<DbPool>,
    action_id: &str,
    user: AuthUser,
) -> Result<Status, Status> {
    let username = user.username.clone();
    let action_id = action_id.to_string();
    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        diesel::update(actions::table.filter(action_id.eq(&action_id)))
            .set(canceled.eq(true))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(
            &mut conn,
            &username,
            "cancel_action",
            Some(&action_id),
            Some("Action canceled"),
        ).map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
    .map(|_| Status::Ok)
}

/// Report a result for an action target
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
        .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Status::Ok)
}
