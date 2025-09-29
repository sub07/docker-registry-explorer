pub mod handler {
    use axum::{http::StatusCode, response::IntoResponse};
    use maud::Markup;

    use crate::error::service;

    pub enum Error {
        NotFound,
        Unknown,
    }

    impl IntoResponse for Error {
        fn into_response(self) -> axum::response::Response {
            match self {
                Self::NotFound => StatusCode::NOT_FOUND,
                Self::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
            }
            .into_response()
        }
    }

    pub type HtmlResult = Result<Markup, Error>;

    impl From<service::Error> for Error {
        fn from(err: service::Error) -> Self {
            match err {
                service::Error::Unknown(_) => Self::Unknown,
            }
        }
    }
}

pub mod service {
    pub enum Error {
        Unknown(anyhow::Error),
    }

    pub type ServiceResult<T> = Result<T, Error>;

    impl<E> From<E> for Error
    where
        E: Into<anyhow::Error>,
    {
        fn from(err: E) -> Self {
            Self::Unknown(err.into())
        }
    }
}
