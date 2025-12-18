use rocket::request::{FromRequest, Outcome, Request};
use rocket::http::Status;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub username: String,
    pub roles: Vec<String>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Example: get username from header "X-User"
        if let Some(user) = req.headers().get_one("X-User") {
            Outcome::Success(AuthUser { username: user.to_string(), roles: vec![] })
        } else {
            Outcome::Failure((Status::Unauthorized, ()))
        }
    }
}
