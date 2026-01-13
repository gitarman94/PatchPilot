use rocket::{get, post, delete, routes, State};
use rocket::form::Form;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use diesel::prelude::*;
use std::collections::HashMap;

use crate::auth::{AuthUser, RoleName};
use crate::db::{DbConn, DbPool, db_log_audit};
use crate::schema::groups::dsl as groups_dsl;
use crate::schema::users::dsl as users_dsl;
use crate::schema::user_groups::dsl as user_groups_dsl;

#[derive(FromForm)]
pub struct UserForm {
    pub username: String,
    pub password: String,
    pub group_id: Option<i32>,
}

#[derive(FromForm)]
pub struct GroupForm {
    pub group_name: String,
    pub description: Option<String>,
}

#[get("/")]
pub fn list_users_groups(user: AuthUser, pool: &State<DbPool>) -> Template {
    if !user.has_role(RoleName::Admin) {
        return Template::render("unauthorized", &());
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("error", &()),
    };

    let all_groups: Vec<(i32, String, Option<String>)> = groups_dsl::groups
        .select((groups_dsl::id, groups_dsl::name, groups_dsl::description))
        .load(&mut conn)
        .unwrap_or_default();

    let joined: Vec<(i32, i32, String)> = users_dsl::users
        .inner_join(
            user_groups_dsl::user_groups.on(
                users_dsl::id.eq(user_groups_dsl::user_id)
            )
        )
        .select((
            user_groups_dsl::group_id,
            users_dsl::id,
            users_dsl::username,
        ))
        .load(&mut conn)
        .unwrap_or_default();

    let mut group_users: HashMap<i32, Vec<(i32, String)>> = HashMap::new();
    for (gid, uid, uname) in joined {
        group_users.entry(gid).or_default().push((uid, uname));
    }

    let context = serde_json::json!({
        "groups": all_groups,
        "group_users": group_users,
    });

    let _ = db_log_audit(&mut conn, &user.username, "users_groups.view", None, None);

    Template::render("users_groups", &context)
}

#[post("/groups/add", data = "<form>")]
pub fn add_group(user: AuthUser, pool: &State<DbPool>, form: Form<GroupForm>) -> Redirect {
    if !user.has_role(RoleName::Admin) {
        return Redirect::to("/unauthorized");
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };

    let ff = form.into_inner();

    let _ = diesel::insert_into(groups_dsl::groups)
        .values((
            groups_dsl::name.eq(&ff.group_name),
            groups_dsl::description.eq(&ff.description),
        ))
        .execute(&mut conn);

    let _ = db_log_audit(
        &mut conn,
        &user.username,
        "group.create",
        Some(&ff.group_name),
        ff.description.as_deref(),
    );

    Redirect::to("/users-groups")
}

#[post("/users/add", data = "<form>")]
pub fn add_user(user: AuthUser, pool: &State<DbPool>, form: Form<UserForm>) -> Redirect {
    if !user.has_role(RoleName::Admin) {
        return Redirect::to("/unauthorized");
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };

    let form = form.into_inner();

    let hashed = match bcrypt::hash(&form.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return Redirect::to("/users-groups"),
    };

    let _ = diesel::insert_into(users_dsl::users)
        .values((
            users_dsl::username.eq(&form.username),
            users_dsl::password_hash.eq(hashed),
        ))
        .execute(&mut conn);

    let new_id: i32 = users_dsl::users
        .order(users_dsl::id.desc())
        .select(users_dsl::id)
        .first(&mut conn)
        .unwrap_or(-1);

    if let Some(gid) = form.group_id {
        let _ = diesel::insert_into(user_groups_dsl::user_groups)
            .values((
                user_groups_dsl::user_id.eq(new_id),
                user_groups_dsl::group_id.eq(gid),
            ))
            .execute(&mut conn);
    }

    let details = form.group_id.map(|gid| format!("assigned_group: {}", gid));
    let _ = db_log_audit(
        &mut conn,
        &user.username,
        "user.create",
        Some(&form.username),
        details.as_deref(),
    );

    Redirect::to("/users-groups")
}

#[delete("/groups/<group_id_val>")]
pub fn delete_group(user: AuthUser, pool: &State<DbPool>, group_id_val: i32) -> Redirect {
    if !user.has_role(RoleName::Admin) {
        return Redirect::to("/unauthorized");
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };

    let group_name: String = groups_dsl::groups
        .filter(groups_dsl::id.eq(group_id_val))
        .select(groups_dsl::name)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".into());

    let _ = diesel::delete(
        user_groups_dsl::user_groups
            .filter(user_groups_dsl::group_id.eq(group_id_val)),
    )
    .execute(&mut conn);

    let _ = diesel::delete(
        groups_dsl::groups
            .filter(groups_dsl::id.eq(group_id_val)),
    )
    .execute(&mut conn);

    let _ = db_log_audit(
        &mut conn,
        &user.username,
        "group.delete",
        Some(&group_name),
        None,
    );

    Redirect::to("/users-groups")
}

#[delete("/users/<user_id_val>")]
pub fn delete_user(user: AuthUser, pool: &State<DbPool>, user_id_val: i32) -> Redirect {
    if !user.has_role(RoleName::Admin) {
        return Redirect::to("/unauthorized");
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };

    let username_val: String = users_dsl::users
        .filter(users_dsl::id.eq(user_id_val))
        .select(users_dsl::username)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".into());

    let _ = diesel::delete(
        user_groups_dsl::user_groups
            .filter(user_groups_dsl::user_id.eq(user_id_val)),
    )
    .execute(&mut conn);

    let _ = diesel::delete(
        users_dsl::users
            .filter(users_dsl::id.eq(user_id_val)),
    )
    .execute(&mut conn);

    let _ = db_log_audit(
        &mut conn,
        &user.username,
        "user.delete",
        Some(&username_val),
        None,
    );

    Redirect::to("/users-groups")
}

pub fn api_users_groups_routes() -> Vec<rocket::Route> {
    routes![
        list_users_groups,
        add_group,
        add_user,
        delete_group,
        delete_user
    ]
}
