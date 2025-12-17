use diesel::prelude::*;
use rocket::{get, post, serde::json::Json, State};
use rocket::http::Status;
use chrono::Utc;

use crate::db::pool::DbPool;
use crate::models::{Action, NewAction, ActionTarget};
use crate::schema::actions::dsl::*;
use crate::schema::action_targets::dsl::*;

/// Submit a new action
#[post("/api/actions", data = "<new_action>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    new_action: Json<NewAction>,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    diesel::insert_into(actions)
        .values(&*new_action)
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::Created)
}

/// List all actions
#[get("/api/actions")]
pub async fn list_actions(pool: &State<DbPool>) -> Result<Json<Vec<Action>>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    let result = actions
        .order(created_at.desc())
        .load::<Action>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(result))
}

/// Cancel an action by ID
#[post("/api/actions/<action_id_param>/cancel")]
pub async fn cancel_action(
    pool: &State<DbPool>,
    action_id_param: &str,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(actions.filter(id.eq(action_id_param.to_string())))
        .set(canceled.eq(true))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}

/// Report a result for an action target
#[post("/api/actions/<_>/result", data = "<result>")]
pub async fn report_action_result(
    pool: &State<DbPool>,
    _ : &str,
    result: Json<ActionTarget>,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(
        action_targets
            .filter(action_id.eq(&result.action_id))
            .filter(device_id.eq(&result.device_id)),
    )
    .set((
        status.eq(&result.status),
        last_update.eq(Utc::now().naive_utc()),
        response.eq(&result.response),
    ))
    .execute(&mut conn)
    .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}
