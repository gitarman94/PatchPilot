use rocket::{get, post, delete, State};
use rocket::form::Form;
use rocket::response::Redirect;
use diesel::prelude::*;
use crate::db::DbPool;
use crate::db::log_audit;
use crate::auth::{AuthUser, UserRole};
use crate::schema::{roles, user_roles};

#[derive(FromForm)]
pub struct RoleForm {
    pub name: String,
}

// List all roles
#[get("/roles")]
pub fn list_roles(user: AuthUser, pool: &State<DbPool>) -> rocket_dyn_templates::Template {
    if !user.has_role(&UserRole::Admin) {
        return rocket_dyn_templates::Template::render("unauthorized", &());
    }

    let conn = pool.get().unwrap();
    let all_roles = roles::table.load::<(i32, String)>(&conn).unwrap_or_default();

    rocket_dyn_templates::Template::render("roles", &all_roles)
}

// Add role
#[post("/roles/add", data = "<form>")]
pub fn add_role(user: AuthUser, pool: &State<DbPool>, form: Form<RoleForm>) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = pool.get().unwrap();
    diesel::insert_into(roles::table)
        .values(roles::name.eq(&form.name))
        .execute(&mut conn)
        .unwrap();

    log_audit(&mut conn, &user.username, "add_role", Some(&form.name), None);

    Redirect::to("/roles")
}

// Delete role
#[delete("/roles/<role_id>")]
pub fn delete_role(user: AuthUser, pool: &State<DbPool>, role_id: i32) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = pool.get().unwrap();
    let role_name = roles::table
        .filter(roles::id.eq(role_id))
        .select(roles::name)
        .first::<String>(&mut conn)
        .unwrap_or_default();

    diesel::delete(user_roles::table.filter(user_roles::role_id.eq(role_id))).execute(&mut conn).unwrap();
    diesel::delete(roles::table.filter(roles::id.eq(role_id))).execute(&mut conn).unwrap();

    log_audit(&mut conn, &user.username, "delete_role", Some(&role_name), None);

    Redirect::to("/roles")
}
