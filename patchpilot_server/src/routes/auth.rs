use rocket::form::Form;
use rocket::http::CookieJar;
use rocket::response::{Redirect, content::RawHtml};
use rocket::State;

use diesel::prelude::*;
use diesel::SelectableHelper;

use std::fs::read_to_string;

use crate::db::DbPool;
use crate::schema::users;

#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct UserRow {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
}

#[post("/login", data = "<form>")]
pub fn login(
    form: Form<LoginForm>,
    cookies: &CookieJar<'_>,
    pool: &State<DbPool>,
) -> Redirect {
    use crate::schema::users::dsl::*;

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/login"),
    };

    let user_opt = users
        .filter(username.eq(&form.username))
        .select(UserRow::as_select())
        .first::<UserRow>(&mut conn)
        .optional()
        .unwrap_or(None);

    if let Some(user) = user_opt {
        let actual_username = user.username.clone(); // now actually read

        if bcrypt::verify(&form.password, &user.password_hash).unwrap_or(false) {
            cookies.add_private(
                rocket::http::Cookie::new("user_id", user.id.to_string())
            );

            // Optionally log successful login in audit
            let _ = crate::routes::history::log_audit(
                &mut conn,
                &actual_username,
                "login",
                None,
                Some("User logged in"),
            );

            return Redirect::to("/dashboard");
        }
    }

    Redirect::to("/login")
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>) -> Redirect {
    cookies.remove_private(rocket::http::Cookie::build("user_id").build());
    Redirect::to("/login")
}

#[get("/login")]
pub fn login_page() -> RawHtml<String> {
    RawHtml(
        read_to_string("templates/login.html")
            .unwrap_or_else(|_| "<h1>Login page missing</h1>".into())
    )
}
