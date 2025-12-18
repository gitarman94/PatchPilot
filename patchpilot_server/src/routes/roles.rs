use rocket::{get, post, delete, State};
use rocket::form::Form;
use rocket::response::Redirect;
use diesel::prelude::*;
use crate::db::{DbPool, log_audit};
use crate::auth::{AuthUser, UserRole};
use crate::schema::{roles, user_roles};
use rocket_dyn_templates::Template;

#[derive(FromForm)]
pub struct RoleForm {
    pub name: String,
}

// List all roles
#[get("/roles")]
pub fn list_roles(user: AuthUser, pool: &State<DbPool>) -> Template {
    if !user.has_role(&UserRole::Admin) {
        return Template::render("unauthorized", &());
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("error", &"DB connection failed"),
    };

    let all_roles = roles::table
        .load::<(i32, String)>(&conn)
        .unwrap_or_default();

    Template::render("roles", &all_roles)
}

// Add role
#[post("/roles/add", data = "<form>")]
pub fn add_role(user: AuthUser, pool: &State<DbPool>, form: Form<RoleForm>) -> Redirect {
    if !user.has_role(&UserRole::Admin) {
        return Redirect::to("/unauthorized");
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/error"),
    };

    if diesel::insert_into(roles::table)
        .values(roles::name.eq(&form.name))
        .execute(&mut conn)
        .is_err()
    {
        return Redirect::to("/error");
    }

    log_audit(&mut conn, &user.username, "add_role", Some(&form.name), None);

    Redirect::to("/roles")
}

// Delete role
#[delete("/roles/<role_id>")]
pub fn delete_role(user: AuthUser, pool: &State<DbPool>, role_id: i32) -> Redirect {
    if !user.has_role(&UserRole::Admin) {
        return Redirect::to("/unauthorized");
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/error"),
    };

    let role_name = roles::table
        .filter(roles::id.eq(role_id))
        .select(roles::name)
        .first::<String>(&mut conn)
        .unwrap_or_else(|_| "unknown".to_string());

    let _ = diesel::delete(user_roles::table.filter(user_roles::role_id.eq(role_id)))
        .execute(&mut conn);
    let _ = diesel::delete(roles::table.filter(roles::id.eq(role_id)))
        .execute(&mut conn);

    log_audit(&mut conn, &user.username, "delete_role", Some(&role_name), None);

    Redirect::to("/roles")
}
