use rocket::form::Form;
use rocket::http::{Cookie, CookieJar, SameSite, Status};
use rocket::response::{Redirect, content::RawHtml};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;

use diesel::prelude::*;

use crate::db::DbPool;
use crate::schema::{audit, users};

use bcrypt::verify;
use chrono::Utc;
use std::fs::read_to_string;

/// Login form structure
#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// Diesel queryable user row
#[derive(Queryable, Clone, Debug)]
pub struct UserRow {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
    pub role: String, // add role to match database
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
    /// Log an action to the audit table
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

    /// Check if the user has a given role
    pub fn has_role(&self, role: UserRole) -> bool {
        match role {
            UserRole::Admin => self.role == "Admin",
            UserRole::User => true, // all users have User role implicitly
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

        use crate::schema::users::dsl::{id as col_id, username as col_username, role as col_role};

        match users
            .filter(col_id.eq(user_id))
            .select((col_id, col_username, col_role))
            .first::<(i32, String, String)>(&mut conn)
        {
            Ok((uid, uname, urole)) => Outcome::Success(AuthUser { id: uid, username: uname, role: urole }),
            Err(_) => Outcome::Failure((Status::Unauthorized, ())),
        }
    }
}

/// Handle login POST
#[post("/login", data = "<form>")]
pub fn login(form: Form<LoginForm>, cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/login"),
    };

    use crate::schema::users::dsl::{id as col_id, password_hash as col_password_hash, role as col_role, username as col_username};

    let user = match users
        .filter(col_username.eq(&form.username))
        .select((col_id, col_username, col_password_hash, col_role))
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

    let auth_user = AuthUser { id: user.id, username: user.username.clone(), role: user.role.clone() };
    auth_user.audit(&mut conn, "login", None);

    Redirect::to("/dashboard")
}

/// Handle logout
#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    let user_id = cookies
        .get_private("user_id")
        .and_then(|c| c.value().parse::<i32>().ok());

    cookies.remove_private(Cookie::from("user_id"));

    if let Some(uid) = user_id {
        if let Ok(mut conn) = pool.get() {
            use crate::schema::users::dsl::{id as col_id, username as col_username, role as col_role};
            if let Ok((uname, urole)) = users
                .filter(col_id.eq(uid))
                .select((col_username, col_role))
                .first::<(String, String)>(&mut conn)
            {
                let auth_user = AuthUser { id: uid, username: uname, role: urole };
                auth_user.audit(&mut conn, "logout", None);
            }
        }
    }

    Redirect::to("/login")
}

/// Serve login page
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
