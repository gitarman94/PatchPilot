use rocket::form::Form;
use rocket::http::CookieJar;
use rocket::response::{Redirect, content::RawHtml};
use rocket::State;
use diesel::prelude::*;
use diesel::SelectableHelper;
use crate::db::DbPool;
use crate::schema::{users, user_actions};
use bcrypt::verify;
use std::fs::read_to_string;

/// Login form structure
#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// Queryable user row
#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UserRow {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
}

/// Authenticated user structure
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
}

impl AuthUser {
    /// Log a user action into the database
    pub fn log_user_action(&self, conn: &mut SqliteConnection, action: &str, target: Option<&str>) {
        let target_str = target.unwrap_or("");
        let _ = diesel::insert_into(user_actions::table)
            .values((
                user_actions::user_id.eq(self.id),
                user_actions::action.eq(action),
                user_actions::target.eq(target_str),
            ))
            .execute(conn);
    }
}

/// Handle login POST
#[post("/login", data = "<form>")]
pub fn login(
    form: Form<LoginForm>,
    cookies: &CookieJar<'_>,
    pool: &State<DbPool>,
) -> Redirect {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/login"),
    };

    use crate::schema::users::dsl::*;

    let user_opt = users
        .filter(username.eq(&form.username))
        .select(UserRow::as_select())
        .first::<UserRow>(&mut conn)
        .optional()
        .unwrap_or(None);

    if let Some(user) = user_opt {
        if verify(&form.password, &user.password_hash).unwrap_or(false) {
            // Set cookie
            cookies.add_private(rocket::http::Cookie::new("user_id", user.id.to_string()));

            // Log successful login
            let auth_user = AuthUser { id: user.id, username: user.username.clone() };
            auth_user.log_user_action(&mut conn, "login", None);

            return Redirect::to("/dashboard");
        }
    }

    Redirect::to("/login")
}

/// Handle logout
#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>) -> Redirect {
    cookies.remove_private(rocket::http::Cookie::build("user_id").finish());
    Redirect::to("/login")
}

/// Serve login page
#[get("/login")]
pub fn login_page() -> RawHtml<String> {
    RawHtml(
        read_to_string("templates/login.html")
            .unwrap_or_else(|_| "<h1>Login page missing</h1>".into())
    )
}
