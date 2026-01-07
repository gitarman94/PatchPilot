use rocket::{get, post, delete, State};
use rocket::form::Form;
use rocket::response::Redirect;
use diesel::prelude::*;
use std::collections::HashMap;

use crate::db::DbPool;
use crate::auth::{AuthUser, UserRole};
use crate::schema::{users, groups, user_groups};
use crate::routes::history::log_audit;

#[derive(FromForm)]
pub struct UserForm {
    pub username: String,
    pub password: String,
    pub group_id: Option<i32>,
}

#[derive(FromForm)]
pub struct GroupForm {
    pub name: String,
    pub description: Option<String>,
}

#[get("/users-groups")]
pub fn list_users_groups(
    user: AuthUser,
    pool: &State<DbPool>,
) -> rocket_dyn_templates::Template {
    if !user.has_role(UserRole::Admin) {
        return rocket_dyn_templates::Template::render("unauthorized", &());
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return rocket_dyn_templates::Template::render("error", &()),
    };

    let all_groups = groups::table
        .load::<(i32, String, Option<String>)>(&mut conn)
        .unwrap_or_default();

    let joined = users::table
        .inner_join(user_groups::table.on(users::id.eq(user_groups::user_id)))
        .select((user_groups::group_id, users::id, users::username))
        .load::<(i32, i32, String)>(&mut conn)
        .unwrap_or_default();

    let mut group_users: HashMap<i32, Vec<(i32, String)>> = HashMap::new();
    for (gid, uid, uname) in joined {
        group_users.entry(gid).or_default().push((uid, uname));
    }

    let context = serde_json::json!({
        "groups": all_groups,
        "group_users": group_users,
    });

    // Audit log: viewing users/groups page
    let _ = log_audit(&mut conn, &user.username, "users_groups.view", None, None);

    rocket_dyn_templates::Template::render("users_groups", &context)
}

#[post("/groups/add", data = "<form>")]
pub fn add_group(
    user: AuthUser,
    pool: &State<DbPool>,
    form: Form<GroupForm>,
) -> Redirect {
    if !user.has_role(UserRole::Admin) {
        return Redirect::to("/unauthorized");
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };

    let _ = diesel::insert_into(groups::table)
        .values((groups::name.eq(&form.name), groups::description.eq(&form.description)))
        .execute(&mut conn);

    let _ = log_audit(
        &mut conn,
        &user.username,
        "group.create",
        Some(&form.name),
        form.description.as_deref(),
    );

    Redirect::to("/users-groups")
}

#[post("/users/add", data = "<form>")]
pub fn add_user(
    user: AuthUser,
    pool: &State<DbPool>,
    form: Form<UserForm>,
) -> Redirect {
    if !user.has_role(UserRole::Admin) {
        return Redirect::to("/unauthorized");
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };

    let hashed = match bcrypt::hash(&form.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return Redirect::to("/users-groups"),
    };

    let new_user = (
        users::username.eq(&form.username),
        users::password_hash.eq(hashed),
    );

    let _ = diesel::insert_into(users::table)
        .values(&new_user)
        .execute(&mut conn);

    let new_id: i32 = users::table
        .order(users::id.desc())
        .select(users::id)
        .first(&mut conn)
        .unwrap_or(-1);

    if let Some(gid) = form.group_id {
        let _ = diesel::insert_into(user_groups::table)
            .values((user_groups::user_id.eq(new_id), user_groups::group_id.eq(gid)))
            .execute(&mut conn);
    }

    let details = form.group_id.map(|gid| format!("assigned_group: {}", gid));
    let details_ref = details.as_deref();

    let _ = log_audit(
        &mut conn,
        &user.username,
        "user.create",
        Some(&form.username),
        details_ref,
    );

    Redirect::to("/users-groups")
}

#[delete("/groups/<group_id_val>")]
pub fn delete_group(
    user: AuthUser,
    pool: &State<DbPool>,
    group_id_val: i32,
) -> Redirect {
    if !user.has_role(UserRole::Admin) {
        return Redirect::to("/unauthorized");
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };

    let group_name: String = groups::table
        .filter(groups::id.eq(group_id_val))
        .select(groups::name)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".into());

    let _ = diesel::delete(user_groups::table.filter(user_groups::group_id.eq(group_id_val)))
        .execute(&mut conn);

    let _ = diesel::delete(groups::table.filter(groups::id.eq(group_id_val)))
        .execute(&mut conn);

    let _ = log_audit(
        &mut conn,
        &user.username,
        "group.delete",
        Some(&group_name),
        None,
    );

    Redirect::to("/users-groups")
}

#[delete("/users/<user_id_val>")]
pub fn delete_user(
    user: AuthUser,
    pool: &State<DbPool>,
    user_id_val: i32,
) -> Redirect {
    if !user.has_role(UserRole::Admin) {
        return Redirect::to("/unauthorized");
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };

    let username_val: String = users::table
        .filter(users::id.eq(user_id_val))
        .select(users::username)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".into());

    let _ = diesel::delete(user_groups::table.filter(user_groups::user_id.eq(user_id_val)))
        .execute(&mut conn);

    let _ = diesel::delete(users::table.filter(users::id.eq(user_id_val)))
        .execute(&mut conn);

    let _ = log_audit(
        &mut conn,
        &user.username,
        "user.delete",
        Some(&username_val),
        None,
    );

    Redirect::to("/users-groups")
}
