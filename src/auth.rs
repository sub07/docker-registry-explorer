pub mod middleware {
    use std::convert::Infallible;

    use axum::{RequestPartsExt, extract::OptionalFromRequestParts};
    use axum_extra::extract::CookieJar;
    use joy_error::ResultInfallibleExt;

    use crate::auth::{self, service};

    pub struct Authenticated;

    impl<S> OptionalFromRequestParts<S> for Authenticated
    where
        S: Send + Sync,
    {
        type Rejection = Infallible;

        async fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            _: &S,
        ) -> Result<Option<Self>, Self::Rejection> {
            let cookie_jar = parts.extract::<CookieJar>().await.unwrap_infallible();
            let token = cookie_jar.get(service::AUTH_TOKEN_COOKIE_NAME);
            token.map_or(Ok(None), |token| {
                let (good_username, good_password) = auth::service::get_credentials();
                let hash = auth::service::hash_credentials(&good_username, &good_password);
                Ok((token.value() == hash).then_some(Self))
            })
        }
    }
}

pub mod dto {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct LoginForm {
        pub username: String,
        pub password: String,
    }

    #[derive(Deserialize)]
    pub struct LoginQuery {
        pub error: Option<String>,
    }
}

pub mod handler {
    use axum::{Form, extract::Query, response::Redirect};
    use axum_extra::extract::{
        CookieJar,
        cookie::{Cookie, SameSite},
    };
    use maud::Markup;
    use time::macros::datetime;

    use crate::auth::{
        dto::{self, LoginForm, LoginQuery},
        middleware::Authenticated,
        service, view,
    };

    pub async fn login_index(
        Query(LoginQuery { error }): Query<LoginQuery>,
        authenticated: Option<Authenticated>,
    ) -> Result<Markup, Redirect> {
        if authenticated.is_some() {
            Err(Redirect::to("/"))
        } else {
            Ok(view::login_index(error))
        }
    }

    pub async fn logout(cookie_jar: CookieJar) -> (CookieJar, Redirect) {
        (
            cookie_jar.remove(
                Cookie::build(service::AUTH_TOKEN_COOKIE_NAME)
                    .path("/")
                    .build(),
            ),
            Redirect::to("/"),
        )
    }

    pub async fn authenticate(
        cookie_jar: CookieJar,
        Form(LoginForm { username, password }): Form<dto::LoginForm>,
    ) -> (CookieJar, Redirect) {
        if service::authenticate(&username, &password) {
            let hash = service::hash_credentials(&username, &password);
            (
                cookie_jar.add(
                    Cookie::build((service::AUTH_TOKEN_COOKIE_NAME, hash))
                        .http_only(true)
                        .secure(true)
                        .path("/")
                        .same_site(SameSite::Strict)
                        .expires(datetime!(9999-01-01 0:00 UTC)),
                ),
                Redirect::to("/"),
            )
        } else {
            (
                cookie_jar,
                Redirect::to("/auth/login?error=invalid_credentials"),
            )
        }
    }
}

pub mod service {
    use std::env;

    use sha2::{Digest, Sha256};

    pub const AUTH_TOKEN_COOKIE_NAME: &str = "auth_token";
    pub fn authenticate(username: &str, password: &str) -> bool {
        let (good_username, good_password) = get_credentials();
        username == good_username && password == good_password
    }

    pub fn hash_credentials(username: &str, password: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{username}{password}").as_bytes());
        format!("{:X}", hasher.finalize())
    }

    pub fn get_credentials() -> (String, String) {
        let username =
            env::var("EXPLORER_USERNAME").expect("EXPLORER_USERNAME environment variable not set");
        let password =
            env::var("EXPLORER_PASSWORD").expect("EXPLORER_PASSWORD environment variable not set");
        (username, password)
    }
}

pub mod view {
    use maud::{Markup, html};

    use crate::common;

    pub fn login_index(error: Option<String>) -> Markup {
        html! {
            html {
                (common::view::head())
                body {
                    (common::view::header())

                    @if let Some(error) = error {
                        .alert .alert-danger .m-2 {
                            "Error: "
                            (error)
                        }
                    }

                    form .my-2 .mx-auto .d-flex .flex-column .gap-2 .col-3 .justify-content-center .align-items-end method="post" action="/auth/authenticate" {
                        input .form-control type="text" name="username" placeholder="Username";
                        input .form-control type="password" name="password" placeholder="Password";
                        button .min-content .btn .btn-primary type="submit" {
                            "Login"
                        }
                    }
                }
            }
        }
    }
}
