use rocket::form::Form;
use rocket::http::{Cookie, CookieJar, SameSite, Status};
use rocket::response::{Redirect, content::RawHtml};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;

use diesel::prelude::*;

use crate::db::DbPool;
use crate::schema::{users, audit};

use bcrypt::verify;
use chrono::Utc;
use std::fs::read_to_string;

#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Queryable, Clone, Debug)]
pub struct UserRow {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let cookies = request.cookies();

        let cookie = match cookies.get_private("user_id") {
            Some(c) => c,
            None => return Outcome::Error((Status::Unauthorized, ())),
        };

        let user_id = match cookie.value().parse::<i32>() {
            Ok(v) => v,
            Err(_) => return Outcome::Error((Status::Unauthorized, ())),
        };

        let pool = match request.guard::<&State<DbPool>>().await {
            Outcome::Success(p) => p,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return Outcome::Error((Status::InternalServerError, ())),
        };

        use crate::schema::users::dsl::{users, id as col_id, username as col_username};

        match users
            .filter(col_id.eq(user_id))
            .select((col_id, col_username))
            .first::<(i32, String)>(&mut conn)
        {
            Ok((uid, uname)) => Outcome::Success(AuthUser { id: uid, username: uname }),
            Err(_) => Outcome::Error((Status::Unauthorized, ())),
        }
    }
}

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

    use crate::schema::users::dsl::{users, username as col_username};

    let user = match users
        .filter(col_username.eq(&form.username))
        .first::<UserRow>(&mut conn)
        .optional()
    {
        Ok(Some(u)) => u,
        _ => return Redirect::to("/login"),
    };

    if !verify(&form.password, &user.password_hash).unwrap_or(false) {
        return Redirect::to("/login");
    }

    let mut cookie = Cookie::new("user_id", user.id.to_string());
    cookie.set_same_site(SameSite::Lax);
    cookies.add_private(cookie);

    let _ = diesel::insert_into(audit::table)
        .values((
            audit::actor.eq(user.username.clone()),
            audit::action_type.eq("login"),
            audit::target.eq::<Option<String>>(None),
            audit::details.eq::<Option<String>>(None),
            audit::created_at.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn);

    Redirect::to("/dashboard")
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    let user_id = cookies
        .get_private("user_id")
        .and_then(|c| c.value().parse::<i32>().ok());

    cookies.remove_private(Cookie::from("user_id"));

    if let Some(uid) = user_id {
        if let Ok(mut conn) = pool.get() {
            use crate::schema::users::dsl::{users, id as col_id, username as col_username};

            if let Ok(uname) = users
                .filter(col_id.eq(uid))
                .select(col_username)
                .first::<String>(&mut conn)
            {
                let _ = diesel::insert_into(audit::table)
                    .values((
                        audit::actor.eq(uname),
                        audit::action_type.eq("logout"),
                        audit::target.eq::<Option<String>>(None),
                        audit::details.eq::<Option<String>>(None),
                        audit::created_at.eq(Utc::now().naive_utc()),
                    ))
                    .execute(&mut conn);
            }
        }
    }

    Redirect::to("/login")
}

#[get("/login")]
pub fn login_page() -> RawHtml<String> {
    RawHtml(
        read_to_string("templates/login.html")
            .unwrap_or_else(|_| "<h1>Login page missing</h1>".to_string()),
    )
}
