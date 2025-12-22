use rocket::{get, post, delete, State};
use rocket::form::Form;
use rocket::response::Redirect;
use diesel::prelude::*;
use crate::db::{DbPool, log_audit};
use crate::auth::{AuthUser, UserRole};
use crate::schema::{users, groups, user_groups};

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
pub fn list_users_groups(user: AuthUser, pool: &State<DbPool>) -> rocket_dyn_templates::Template {
    if !user.has_role(&UserRole::Admin) {
        return rocket_dyn_templates::Template::render("unauthorized", &());
    }

    let mut conn = pool.get().expect("Failed to get DB connection");

    let all_groups = groups::table
        .load::<(i32, String, Option<String>)>(&mut conn)
        .unwrap_or_default();

    let mut group_users: std::collections::HashMap<i32, Vec<(i32, String)>> = std::collections::HashMap::new();
    let joined = users::table
        .inner_join(user_groups::table.on(users::id.eq(user_groups::user_id)))
        .select((user_groups::group_id, users::id, users::username))
        .load::<(i32, i32, String)>(&mut conn)
        .unwrap_or_default();

    for (group_id_val, user_id_val, username_val) in joined {
        group_users.entry(group_id_val).or_default().push((user_id_val, username_val));
    }

    let context = serde_json::json!({
        "groups": all_groups,
        "group_users": group_users
    });

    rocket_dyn_templates::Template::render("users_groups", &context)
}

#[post("/groups/add", data = "<form>")]
pub fn add_group(user: AuthUser, pool: &State<DbPool>, form: Form<GroupForm>) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to get DB connection: {}", e);
            return Redirect::to("/users-groups");
        }
    };

    if let Err(e) = diesel::insert_into(groups::table)
        .values((groups::name.eq(&form.name), groups::description.eq(&form.description)))
        .execute(&mut conn)
    {
        eprintln!("Failed to insert group: {}", e);
    }

    if let Err(e) = log_audit(&mut conn, &user.username, "add_group", Some(&form.name), form.description.as_deref()) {
        eprintln!("Audit log failed for add_group: {}", e);
    }

    Redirect::to("/users-groups")
}

#[post("/users/add", data = "<form>")]
pub fn add_user(user: AuthUser, pool: &State<DbPool>, form: Form<UserForm>) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to get DB connection: {}", e);
            return Redirect::to("/users-groups");
        }
    };

    let pass_hash = match bcrypt::hash(&form.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to hash password: {}", e);
            return Redirect::to("/users-groups");
        }
    };

    let new_user = (
        users::username.eq(&form.username),
        users::password_hash.eq(pass_hash),
    );

    if let Err(e) = diesel::insert_into(users::table)
        .values(&new_user)
        .execute(&mut conn)
    {
        eprintln!("Failed to insert user: {}", e);
        return Redirect::to("/users-groups");
    }

    let user_id_val: i32 = users::table
        .order(users::id.desc())
        .select(users::id)
        .first(&mut conn)
        .unwrap_or(-1);

    if let Some(group_id_val) = form.group_id {
        if let Err(e) = diesel::insert_into(user_groups::table)
            .values((user_groups::user_id.eq(user_id_val), user_groups::group_id.eq(group_id_val)))
            .execute(&mut conn)
        {
            eprintln!("Failed to assign user to group: {}", e);
        }
    }

    let details = form.group_id.map(|id| format!("group_id: {}", id));
    let details_ref = details.as_deref();

    if let Err(e) = log_audit(&mut conn, &user.username, "add_user", Some(&form.username), details_ref) {
        eprintln!("Audit log failed for add_user: {}", e);
    }

    Redirect::to("/users-groups")
}

#[delete("/groups/<group_id_val>")]
pub fn delete_group(user: AuthUser, pool: &State<DbPool>, group_id_val: i32) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to get DB connection: {}", e);
            return Redirect::to("/users-groups");
        }
    };

    let group_name_val: String = groups::table
        .filter(groups::id.eq(group_id_val))
        .select(groups::name)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".into());

    if let Err(e) = diesel::delete(user_groups::table.filter(user_groups::group_id.eq(group_id_val)))
        .execute(&mut conn)
    {
        eprintln!("Failed to delete user_groups: {}", e);
    }

    if let Err(e) = diesel::delete(groups::table.filter(groups::id.eq(group_id_val)))
        .execute(&mut conn)
    {
        eprintln!("Failed to delete group: {}", e);
    }

    if let Err(e) = log_audit(&mut conn, &user.username, "delete_group", Some(&group_name_val), None) {
        eprintln!("Audit log failed for delete_group: {}", e);
    }

    Redirect::to("/users-groups")
}

#[delete("/users/<user_id_val>")]
pub fn delete_user(user: AuthUser, pool: &State<DbPool>, user_id_val: i32) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to get DB connection: {}", e);
            return Redirect::to("/users-groups");
        }
    };

    let username_val: String = users::table
        .filter(users::id.eq(user_id_val))
        .select(users::username)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".into());

    if let Err(e) = diesel::delete(user_groups::table.filter(user_groups::user_id.eq(user_id_val)))
        .execute(&mut conn)
    {
        eprintln!("Failed to delete user_groups for user: {}", e);
    }

    if let Err(e) = diesel::delete(users::table.filter(users::id.eq(user_id_val)))
        .execute(&mut conn)
    {
        eprintln!("Failed to delete user: {}", e);
    }

    if let Err(e) = log_audit(&mut conn, &user.username, "delete_user", Some(&username_val), None) {
        eprintln!("Audit log failed for delete_user: {}", e);
    }

    Redirect::to("/users-groups")
}
