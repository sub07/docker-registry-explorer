pub mod dto {
    pub struct Image {
        pub name: String,
        pub tag_count: usize,
    }
}

pub mod handler {
    use axum::{extract::State, response::Redirect};
    use maud::Markup;

    use crate::{
        AppState,
        auth::middleware::Authenticated,
        home::{service, view},
    };

    pub async fn index(
        State(AppState {
            registry_api_client,
            ..
        }): State<AppState>,
        auth: Option<Authenticated>,
    ) -> Result<Markup, Redirect> {
        if auth.is_none() {
            return Err(Redirect::to("/auth/login"));
        }
        let Ok(images) = service::get_images(registry_api_client).await else {
            return Ok(view::index(&view::error("Could not retrieve images")));
        };
        Ok(view::index(&view::image_table(images)))
    }
}

pub mod service {
    use joy_error::ResultLogExt;

    use crate::{error::service::ServiceResult, home::dto::Image, registry};

    #[tracing::instrument]
    pub async fn get_images(
        registry_api_client: registry::api::Client,
    ) -> ServiceResult<Vec<Image>> {
        let images = registry_api_client.catalog().await.log_err()?.repositories;

        let tag_counts = futures::future::join_all(
            images
                .iter()
                .map(|image| registry_api_client.count_tags(image)),
        )
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()
        .log_err()?;

        let images = images
            .into_iter()
            .zip(tag_counts)
            .map(|(image, tag_count)| Image {
                name: image,
                tag_count,
            })
            .collect();

        Ok(images)
    }
}

pub mod view {
    use maud::{Markup, html};

    use crate::{common, home::dto::Image};

    pub fn index(body: &Markup) -> Markup {
        html! {
            html {
                (common::view::head())
                body {
                    (common::view::header())
                    (body)
                }
            }
        }
    }

    pub fn error(message: &str) -> Markup {
        html! {
            div .alert .alert-danger {
                (message)
            }
        }
    }

    pub fn image_table(images: Vec<Image>) -> Markup {
        html! {
            table .table .table-striped .table-bordered .table-hover .table-responsive {
                thead {
                    tr {
                        th { "Image Name" }
                        th { "Tag Count" }
                    }
                }
                tbody {
                    @for image in images {
                        @if image.tag_count > 0 {
                            tr {
                                td { a href=(image.name) { (image.name) } }
                                td { (image.tag_count) }
                            }
                        }
                    }
                }
            }
        }
    }
}
