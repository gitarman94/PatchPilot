use diesel::prelude::*;
use rocket::{get, post, serde::json::Json, State};
use rocket::http::Status;
use chrono::Utc;

use crate::db::pool::DbPool;
use crate::models::{Action, NewAction, ActionTarget};
use crate::schema::actions::dsl::*;
use crate::schema::action_targets::dsl::*;

/// Submit a new action
#[post("/api/actions", data = "<action>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    action: Json<NewAction>,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    diesel::insert_into(actions::table)
        .values(&*action)
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::Created)
}

/// List all actions
#[get("/api/actions")]
pub async fn list_actions(pool: &State<DbPool>) -> Result<Json<Vec<Action>>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    let result = actions::table
        .order(actions::created_at.desc())
        .load::<Action>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    Ok(Json(result))
}

/// Cancel an action by ID
#[post("/api/actions/<action_id>")]
pub async fn cancel_action(
    pool: &State<DbPool>,
    action_id: &str,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    diesel::update(actions::table.filter(actions::id.eq(action_id)))
        .set(actions::canceled.eq(true))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    Ok(Status::Ok)
}

/// Report a result for an action target
#[post("/api/actions/<_>/result", data = "<result>")]
pub async fn report_action_result(
    pool: &State<DbPool>,
    _: &str,
    result: Json<ActionTarget>,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(
        action_targets::table
            .filter(action_targets::action_id.eq(&result.action_id))
            .filter(action_targets::target.eq(&result.target)),
    )
    .set((
        action_targets::status.eq(&result.status),
        action_targets::last_update.eq(chrono::Utc::now().naive_utc()),
        action_targets::response.eq(&result.response),
    ))
    .execute(&mut conn)
    .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}
