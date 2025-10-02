pub mod middleware {
    use axum::{RequestPartsExt, extract::FromRequestParts, response::Redirect};
    use axum_extra::extract::CookieJar;
    use joy_error::ResultInfallibleExt;

    use crate::auth::{self, service::CookieJarExtUtils};

    pub struct Authenticated;

    impl<S> FromRequestParts<S> for Authenticated
    where
        S: Send + Sync,
    {
        type Rejection = (CookieJar, Redirect);

        async fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            _: &S,
        ) -> Result<Self, Self::Rejection> {
            let cookie_jar = parts.extract::<CookieJar>().await.unwrap_infallible();
            let token = cookie_jar.get_auth_token();
            let redirection = || Redirect::to(&format!("/auth/login?from={}", parts.uri.path()));
            if let Some(token) = token {
                let (good_username, good_password) = auth::service::get_credentials();
                let hash = auth::service::hash_credentials(&good_username, &good_password);
                (token == hash)
                    .then_some(Self)
                    .ok_or_else(|| (cookie_jar.remove_auth_token(), redirection()))
            } else {
                Err((cookie_jar, redirection()))
            }
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
        pub error: Option<LoginError>,
        pub from: Option<String>,
        pub username: Option<String>,
    }

    #[derive(Deserialize)]
    pub struct AuthenticateQuery {
        pub from: Option<String>,
    }

    #[derive(Deserialize)]
    pub enum LoginError {
        #[serde(rename = "invalid_credentials")]
        InvalidCredentials,
    }
}

pub mod handler {
    use axum::{Form, extract::Query, response::Redirect};
    use axum_extra::extract::CookieJar;
    use maud::Markup;

    use crate::auth::{
        dto::{self, AuthenticateQuery, LoginForm, LoginQuery},
        service::{self, CookieJarExtUtils},
        view,
    };

    pub async fn login_index(
        Query(LoginQuery {
            error,
            from,
            username,
        }): Query<LoginQuery>,
    ) -> Result<Markup, Redirect> {
        Ok(view::login_index(error, from, username.as_deref()))
    }

    pub async fn logout(cookie_jar: CookieJar) -> (CookieJar, Redirect) {
        (cookie_jar.remove_auth_token(), Redirect::to("/"))
    }

    pub async fn authenticate(
        cookie_jar: CookieJar,
        Query(AuthenticateQuery { from }): Query<AuthenticateQuery>,
        Form(LoginForm { username, password }): Form<dto::LoginForm>,
    ) -> (CookieJar, Redirect) {
        if service::authenticate(&username, &password) {
            let hash = service::hash_credentials(&username, &password);
            (
                cookie_jar.set_auth_token(hash),
                Redirect::to(from.as_deref().unwrap_or("/")),
            )
        } else {
            (
                cookie_jar,
                Redirect::to(&format!(
                    "/auth/login?error=invalid_credentials&username={username}{}",
                    from.map_or(String::new(), |from| format!("&from={from}"))
                )),
            )
        }
    }
}

pub mod service {
    use std::env;

    use axum_extra::extract::{
        CookieJar,
        cookie::{Cookie, SameSite},
    };
    use sha2::{Digest, Sha256};
    use time::macros::datetime;

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

    #[easy_ext::ext(CookieJarExtUtils)]
    impl CookieJar {
        pub fn set_auth_token(self, token: String) -> Self {
            self.add(
                Cookie::build((AUTH_TOKEN_COOKIE_NAME, token))
                    .http_only(true)
                    .secure(true)
                    .path("/")
                    .same_site(SameSite::Strict)
                    .expires(datetime!(9999-01-01 0:00 UTC)),
            )
        }

        pub fn remove_auth_token(self) -> Self {
            self.remove(Cookie::build(AUTH_TOKEN_COOKIE_NAME).path("/").build())
        }

        pub fn get_auth_token(&self) -> Option<&str> {
            self.get(AUTH_TOKEN_COOKIE_NAME).map(Cookie::value)
        }
    }
}

pub mod view {
    use maud::{Markup, html};

    use crate::{auth::dto::LoginError, common};

    pub fn login_index(
        error: Option<LoginError>,
        from: Option<String>,
        username: Option<&str>,
    ) -> Markup {
        let authenticate_action = from.map_or_else(
            || "/auth/authenticate".to_string(),
            |from| format!("/auth/authenticate?from={from}"),
        );

        let error = error.map(|error| match error {
            LoginError::InvalidCredentials => "Invalid username or password",
        });

        html! {
            html {
                (common::view::head())
                body {
                    (common::view::header())

                    @if let Some(error) = error {
                        .alert .alert-danger .m-2 {
                            (error)
                        }
                    }

                    form .my-2 .mx-auto .d-flex .flex-column .gap-2 .col-3 .justify-content-center .align-items-end method="post" action=(authenticate_action) {
                        input .form-control type="text" name="username" autofocus[username.is_none()] value=[username.as_deref()] placeholder="Username";
                        input .form-control autofocus[username.is_some()] type="password" name="password" placeholder="Password";
                        button .min-content .btn .btn-primary type="submit" {
                            "Login"
                        }
                    }
                }
            }
        }
    }
}
