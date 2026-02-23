pub mod dto {
    pub struct Image {
        pub name: String,
        pub tag_count: usize,
    }
}

pub mod handler {
    use axum::{
        extract::{Path, State},
        response::Redirect,
    };
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
        _: Authenticated,
    ) -> Result<Markup, Redirect> {
        let Ok(images) = service::get_images(registry_api_client).await else {
            return Ok(view::index(view::error("Could not retrieve images")));
        };
        Ok(view::index(view::image_table(images)))
    }

    pub async fn delete_all_image_tags(
        State(AppState {
            registry_api_client,
            ..
        }): State<AppState>,
        _: Authenticated,
        Path(image_name): Path<String>,
    ) -> Redirect {
        let _ = service::delete_all_image_tags(registry_api_client, &image_name).await;
        Redirect::to("/")
    }
}

pub mod service {
    use itertools::Itertools;
    use joy_error::log::ResultLogExt;

    use crate::{
        error::service::ServiceResult,
        home::dto::Image,
        registry::{self, dto::TagManifest},
    };

    pub async fn delete_all_image_tags(
        registry_api_client: registry::api::Client,
        image_name: &str,
    ) -> ServiceResult<()> {
        let tags = registry_api_client
            .tags(image_name)
            .await
            .error()
            .log_err()?;
        if let Some(tags) = tags.tags {
            let digests = futures::future::join_all(
                tags.iter()
                    .map(|tag| registry_api_client.manifest(image_name, tag)),
            )
            .await
            .into_iter()
            .collect::<Result<Vec<TagManifest>, _>>()?
            .into_iter()
            .map(|m| m.digest().to_owned())
            .unique();
            for digest in digests {
                registry_api_client
                    .delete_tag(image_name, &digest)
                    .await
                    .error()
                    .log_err()?;
            }
        }
        Ok(())
    }

    #[tracing::instrument]
    pub async fn get_images(
        registry_api_client: registry::api::Client,
    ) -> ServiceResult<Vec<Image>> {
        let images = registry_api_client
            .catalog()
            .await
            .error()
            .log_err()?
            .repositories;

        let tag_counts = futures::future::join_all(
            images
                .iter()
                .map(|image| registry_api_client.count_tags(image)),
        )
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()
        .error()
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

    pub fn index(body: Markup) -> Markup {
        html! {
            (common::view::page().content(body).call())
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
            table .table .table-striped .table-bordered .table-hover .table-responsive .align-middle .text-center {
                thead {
                    tr {
                        th { "Image Name" }
                        th { "Tag Count" }
                        th { "Action" }
                    }
                }
                tbody {
                    @for image in images {
                        @if image.tag_count > 0 {
                            tr {
                                td { a href=(image.name) { (image.name) } }
                                td { (image.tag_count) }
                                td {
                                    form action=(format!("{}/delete", image.name)) method="post" .m-0 {
                                        button .btn .btn-danger type="submit" {
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
