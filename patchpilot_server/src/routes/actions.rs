use diesel::prelude::*;
use rocket::{get, post, serde::json::Json, State};
use rocket::http::Status;
use chrono::Utc;

use crate::db::pool::DbPool;
use crate::models::{Action, NewAction, ActionTarget};

// Explicitly alias columns to avoid ambiguity
use crate::schema::actions::dsl::{actions, id as action_id_col, created_at, canceled};
use crate::schema::action_targets::dsl::{
    action_targets,
    action_id as at_action_id,
    device_id as at_device_id,
    status as at_status,
    last_update as at_last_update,
    response as at_response
};

/// Submit a new action
#[post("/api/actions", data = "<action>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    action: Json<NewAction>,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let action_data = action.into_inner();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        diesel::insert_into(actions)
            .values(&action_data)
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)?; 

    Ok(Status::Created)
}

/// List all actions
#[get("/api/actions")]
pub async fn list_actions(pool: &State<DbPool>) -> Result<Json<Vec<Action>>, Status> {
    let pool = pool.inner().clone();

    let result = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        actions
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
) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let action_id = action_id.to_string();

    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        diesel::update(actions.filter(action_id_col.eq(&action_id)))
            .set(canceled.eq(true))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)?; 

    Ok(Status::Ok)
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
            action_targets
                .filter(at_action_id.eq(&result.action_id))
                .filter(at_device_id.eq(&result.device_id)),
        )
        .set((
            at_status.eq(&result.status),
            at_last_update.eq(Utc::now().naive_utc()),
            at_response.eq(&result.response),
        ))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)?; 

    Ok(Status::Ok)
}
