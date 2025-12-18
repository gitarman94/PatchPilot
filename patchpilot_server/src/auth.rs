use rocket::request::{FromRequest, Outcome, Request};
use rocket::http::Status;
use rocket::State;
use diesel::prelude::*;
use crate::db::DbPool;
use crate::schema::{users, roles, user_roles};

#[derive(Debug, Clone, PartialEq)]
pub enum UserRole {
    Admin,
    Manager,
    User,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
    pub roles: Vec<UserRole>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let cookie = req.cookies().get_private("user_id");

        let pool = match req.guard::<&State<DbPool>>().await {
            Outcome::Success(p) => p,
            _ => return Outcome::Failure((Status::InternalServerError, ())),
        };

        if let Some(cookie) = cookie {
            if let Ok(user_id) = cookie.value().parse::<i32>() {
                let mut conn = match pool.get() {
                    Ok(c) => c,
                    Err(_) => return Outcome::Failure((Status::InternalServerError, ())),
                };

                // Fetch username
                let username_result = users::table
                    .filter(users::id.eq(user_id))
                    .select(users::username)
                    .first::<String>(&mut conn)
                    .optional()
                    .unwrap_or(None);

                if let Some(username) = username_result {
                    // Fetch roles
                    let role_names = user_roles::table
                        .inner_join(roles::table.on(roles::id.eq(user_roles::role_id)))
                        .filter(user_roles::user_id.eq(user_id))
                        .select(roles::name)
                        .load::<String>(&mut conn)
                        .unwrap_or_else(|_| vec![]);

                    let roles_vec = role_names
                        .into_iter()
                        .map(|r| match r.as_str() {
                            "Admin" => UserRole::Admin,
                            "Manager" => UserRole::Manager,
                            _ => UserRole::User,
                        })
                        .collect();

                    return Outcome::Success(AuthUser {
                        id: user_id,
                        username,
                        roles: roles_vec,
                    });
                }
            }
        }

        Outcome::Failure((Status::Unauthorized, ()))
    }
}

impl AuthUser {
    pub fn has_role(&self, role: &UserRole) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}
