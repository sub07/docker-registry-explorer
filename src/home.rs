pub mod handler {
    use axum::extract::State;

    use crate::{
        AppState,
        error::handler::HtmlResult,
        home::{service, view},
    };

    pub async fn index(
        State(AppState {
            registry_api_client,
            ..
        }): State<AppState>,
    ) -> HtmlResult {
        let images = service::get_images(registry_api_client).await?;
        Ok(view::index(images))
    }
}

pub mod service {
    use joy_error::ResultLogExt;

    use crate::{error::service::ServiceResult, registry};

    pub async fn get_images(
        registry_api_client: registry::api::Client,
    ) -> ServiceResult<Vec<String>> {
        Ok(registry_api_client.catalog().await.log_err()?.repositories)
    }
}

pub mod view {
    use maud::{Markup, html};

    use crate::common;

    pub fn index(images: Vec<String>) -> Markup {
        html! {
            html {
                (common::view::head())
                body {
                    h1 { "Welcome to Docker Registry Explorer" }
                    ul {
                        @for image in images {
                            li { a href=(image) { (image) } }
                        }
                    }
                }
            }
        }
    }
}
