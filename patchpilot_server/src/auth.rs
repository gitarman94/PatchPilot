use rocket::form::Form;
use rocket::http::{Cookie, CookieJar, Status};
use rocket::request::{FromRequest, Request, Outcome as RequestOutcome};
use rocket::response::Redirect;
use rocket::State;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use bcrypt::verify;
use std::fs::read_to_string;
use crate::db::{DbPool, log_audit};
use crate::schema::{users, roles, user_roles};
use diesel::result::QueryResult;

/// Form for login
#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// Authenticated user provided by request guard
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
    pub role: String,
}

/// Role name helper enum (local to auth)
#[derive(Debug, PartialEq, Eq)]
pub enum RoleName {
    Admin,
    User,
}

impl RoleName {
    pub fn as_str(&self) -> &'static str {
        match self {
            RoleName::Admin => "Admin",
            RoleName::User => "User",
        }
    }

    pub fn from_name(name: &str) -> RoleName {
        match name.to_ascii_lowercase().as_str() {
            "admin" => RoleName::Admin,
            _ => RoleName::User,
        }
    }
}

impl AuthUser {
    pub fn has_role(&self, role: RoleName) -> bool {
        match role {
            RoleName::Admin => self.role == RoleName::Admin.as_str(),
            RoleName::User => true,
        }
    }

    pub fn audit(
        &self,
        conn: &mut SqliteConnection,
        action: &str,
        target: Option<&str>,
    ) -> QueryResult<()> {
        let actor = format!("{}:{}", self.id, self.username);
        log_audit(conn, &actor, action, target, None)
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> RequestOutcome<'r, AuthUser, ()> {
        // read user_id cookie (private/encrypted)
        let user_id = match req
            .cookies()
            .get_private("user_id")
            .and_then(|c| c.value().parse::<i32>().ok())
        {
            Some(id) => id,
            None => return RequestOutcome::Failure((Status::Unauthorized, ())),
        };

        // get the DB pool from state
        let pool = match req.guard::<&State<DbPool>>().await {
            RequestOutcome::Success(p) => p,
            _ => return RequestOutcome::Failure((Status::InternalServerError, ())),
        };

        // get a pooled connection
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return RequestOutcome::Failure((Status::InternalServerError, ())),
        };

        // query user + optional role
        let result = users::table
            .filter(users::id.eq(user_id))
            .left_outer_join(user_roles::table.on(user_roles::user_id.eq(users::id)))
            .left_outer_join(roles::table.on(roles::id.eq(user_roles::role_id)))
            .select((users::id, users::username, roles::name.nullable()))
            .first::<(i32, String, Option<String>)>(&mut conn);

        match result {
            Ok((id, username, role_opt)) => {
                let role = role_opt
                    .as_deref()
                    .map(|r| RoleName::from_name(r).as_str().to_string())
                    .unwrap_or_else(|| RoleName::User.as_str().to_string());

                RequestOutcome::Success(AuthUser { id, username, role })
            }
            Err(_) => RequestOutcome::Failure((Status::Unauthorized, ())),
        }
    }
}

#[post("/login", data = "<form>")]
pub fn login(
    form: Form<LoginForm>,
    cookies: &CookieJar<'_>,
    pool: &State<DbPool>,
) -> Redirect {
    // obtain a connection from the pool
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/login"),
    };

    // fetch user + password hash + optional role
    let result = users::table
        .filter(users::username.eq(&form.username))
        .left_outer_join(user_roles::table.on(user_roles::user_id.eq(users::id)))
        .left_outer_join(roles::table.on(roles::id.eq(user_roles::role_id)))
        .select((
            users::id,
            users::username,
            users::password_hash,
            roles::name.nullable(),
        ))
        .first::<(i32, String, String, Option<String>)>(&mut conn);

    let (id, username, hash, role_opt) = match result {
        Ok(r) => r,
        Err(_) => return Redirect::to("/login"),
    };

    if !verify(&form.password, &hash).unwrap_or(false) {
        return Redirect::to("/login");
    }

    // set private cookie
    cookies.add_private(Cookie::new("user_id", id.to_string()));

    let role = role_opt
        .as_deref()
        .map(|r| RoleName::from_name(r).as_str().to_string())
        .unwrap_or_else(|| RoleName::User.as_str().to_string());

    let user = AuthUser { id, username, role };

    // audit but ignore failures
    let _ = user.audit(&mut conn, "login", None);

    Redirect::to("/dashboard")
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    // try to read current cookie
    let user_id_opt = cookies
        .get_private("user_id")
        .and_then(|c| c.value().parse::<i32>().ok());

    // remove cookie
    cookies.remove_private(Cookie::new("user_id", ""));

    if let Some(uid) = user_id_opt {
        if let Ok(mut conn) = pool.get() {
            if let Ok((username, _role_opt)) = users::table
                .filter(users::id.eq(uid))
                .left_outer_join(user_roles::table.on(user_roles::user_id.eq(users::id)))
                .left_outer_join(roles::table.on(roles::id.eq(user_roles::role_id)))
                .select((users::username, roles::name.nullable()))
                .first::<(String, Option<String>)>(&mut conn)
            {
                let actor = format!("{}:{}", uid, username);
                let _ = log_audit(&mut conn, &actor, "logout", None, None);
            }
        }
    }

    Redirect::to("/login")
}

#[get("/login")]
pub fn login_page() -> String {
    read_to_string("templates/login.html").unwrap_or_else(|_| {
        "
# Login page missing

"
        .to_string()
    })
}
