use diesel::prelude::*;
use rocket::{get, post, serde::json::Json, State};
use rocket::http::Status;
use chrono::Utc;

use crate::db::pool::DbPool;
use crate::models::{Action, NewAction, ActionTarget};
use crate::schema::{actions, action_targets};

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

#[get("/api/actions")]
pub async fn list_actions(pool: &State<DbPool>) -> Result<Json<Vec<Action>>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    let rows = actions::table
        .order(actions::created_at.desc())
        .load::<Action>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(rows))
}

#[post("/api/actions/<id>/cancel")]
pub async fn cancel_action(
    pool: &State<DbPool>,
    id: i32, // <--- ID should match schema type
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(actions::table.filter(actions::id.eq(id)))
        .set(actions::status.eq("canceled")) // assuming "canceled" string; update if schema has a boolean column
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}

#[post("/api/actions/<id>/result", data = "<result>")]
pub async fn report_action_result(
    pool: &State<DbPool>,
    id: i32, // match schema type
    result: Json<ActionTarget>,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(
        action_targets::table
            .filter(action_targets::action_id.eq(id))
            .filter(action_targets::target.eq(&result.target)), // use target field instead of device_id if no device_id
    )
    .set((
        action_targets::status.eq(&result.status),
        action_targets::last_update.eq(Utc::now().naive_utc()),
    ))
    .execute(&mut conn)
    .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}
