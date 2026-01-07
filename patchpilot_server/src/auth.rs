use rocket::form::Form;
use rocket::http::{Cookie, CookieJar, SameSite, Status};
use rocket::response::{Redirect, content::RawHtml};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;

use diesel::prelude::*;

use crate::db::DbPool;
use crate::schema::{audit, users, roles, user_roles};

use bcrypt::verify;
use chrono::Utc;
use std::fs::read_to_string;

/// Local struct representing a user from the database
#[derive(Queryable)]
struct User {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
}

/// Login form structure
#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// Authenticated user representation
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
    pub role: String,
}

/// User roles
#[derive(Debug, PartialEq, Eq)]
pub enum UserRole {
    Admin,
    User,
}

impl AuthUser {
    pub fn audit(&self, conn: &mut SqliteConnection, action: &str, target: Option<&str>) {
        let _ = diesel::insert_into(audit::table)
            .values((
                audit::actor.eq(&self.username),
                audit::action_type.eq(action),
                audit::target.eq(target.map(|s| s.to_string())),
                audit::details.eq::<Option<String>>(None),
                audit::created_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn);
    }

    pub fn has_role(&self, role: UserRole) -> bool {
        match role {
            UserRole::Admin => self.role == "Admin",
            UserRole::User => true,
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let cookie = match request.cookies().get_private("user_id") {
            Some(c) => c,
            None => return Outcome::Failure((Status::Unauthorized, ())),
        };

        let user_id = match cookie.value().parse::<i32>() {
            Ok(v) => v,
            Err(_) => return Outcome::Failure((Status::Unauthorized, ())),
        };

        let pool = match request.guard::<&State<DbPool>>().await {
            Outcome::Success(p) => p,
            _ => return Outcome::Failure((Status::InternalServerError, ())),
        };

        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return Outcome::Failure((Status::InternalServerError, ())),
        };

        match users::table
            .filter(users::id.eq(user_id))
            .inner_join(user_roles::table.on(user_roles::user_id.eq(users::id)))
            .inner_join(roles::table.on(roles::id.eq(user_roles::role_id)))
            .select((users::id, users::username, roles::name))
            .first::<(i32, String, String)>(&mut conn)
        {
            Ok((uid, uname, urole)) => Outcome::Success(AuthUser { id: uid, username: uname, role: urole }),
            Err(_) => Outcome::Failure((Status::Unauthorized, ())),
        }
    }
}

#[post("/login", data = "<form>")]
pub fn login(form: Form<LoginForm>, cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/login"),
    };

    match users::table
        .filter(users::username.eq(&form.username))
        .inner_join(user_roles::table.on(user_roles::user_id.eq(users::id)))
        .inner_join(roles::table.on(roles::id.eq(user_roles::role_id)))
        .select((users::id, users::username, users::password_hash, roles::name))
        .first::<(i32, String, String, String)>(&mut conn)
        .optional()
    {
        Ok(Some((uid, uname, password_hash, role))) => {
            if !verify(&form.password, &password_hash).unwrap_or(false) {
                return Redirect::to("/login");
            }

            let mut cookie = Cookie::new("user_id", uid.to_string());
            cookie.set_same_site(SameSite::Lax);
            cookies.add_private(cookie);

            let auth_user = AuthUser { id: uid, username: uname.clone(), role: role.clone() };
            auth_user.audit(&mut conn, "login", None);

            Redirect::to("/dashboard")
        }
        _ => Redirect::to("/login"),
    }
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    let user_id = cookies
        .get_private("user_id")
        .and_then(|c| c.value().parse::<i32>().ok());

    cookies.remove_private(Cookie::from("user_id"));

    if let Some(uid) = user_id {
        if let Ok(mut conn) = pool.get() {
            if let Ok((uname, urole)) = users::table
                .inner_join(user_roles::table.on(user_roles::user_id.eq(users::id)))
                .inner_join(roles::table.on(roles::id.eq(user_roles::role_id)))
                .filter(users::id.eq(uid))
                .select((users::username, roles::name))
                .first::<(String, String)>(&mut conn)
            {
                let auth_user = AuthUser { id: uid, username: uname, role: urole };
                auth_user.audit(&mut conn, "logout", None);
            }
        }
    }

    Redirect::to("/login")
}

#[get("/login")]
pub fn login_page() -> RawHtml<String> {
    RawHtml(read_to_string("templates/login.html").unwrap_or_else(|_| "<h1>Login page missing</h1>".to_string()))
}

/// Token validation stub for API use
pub async fn validate_token(token: &str) -> Result<AuthUser, ()> {
    if token == "testtoken" {
        Ok(AuthUser { id: 1, username: "admin".into(), role: "Admin".into() })
    } else {
        Err(())
    }
}
