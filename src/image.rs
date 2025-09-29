pub mod dto {
    pub struct ImageInfo {
        pub tags: Vec<Tag>,
    }

    pub struct Tag {
        pub name: String,
        pub digest: String,
    }
}

pub mod handler {
    use axum::extract::{Path, State};

    use crate::{
        AppState,
        error::handler::HtmlResult,
        image::{service, view},
    };

    pub async fn index(
        Path(image): Path<String>,
        State(AppState {
            registry_api_client,
            ..
        }): State<AppState>,
    ) -> HtmlResult {
        let info = service::get_image_info(registry_api_client, &image).await?;
        Ok(view::index(&image, &info))
    }
}

pub mod service {
    use joy_error::ResultLogExt;

    use crate::{
        error::service::ServiceResult,
        image::dto::{ImageInfo, Tag},
        registry,
    };

    pub async fn get_image_info(
        registry_api_client: registry::api::Client,
        image_name: &str,
    ) -> ServiceResult<ImageInfo> {
        let tags = registry_api_client.tags(image_name).await.log_err()?.tags;
        let digests = futures::future::join_all(
            tags.iter()
                .map(|tag| registry_api_client.digest(image_name, tag)),
        )
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<String>>>()?;

        let tags = tags
            .into_iter()
            .zip(digests.into_iter())
            .map(|(tag, digest)| Tag { name: tag, digest })
            .collect();

        Ok(ImageInfo { tags })
    }
}

pub mod view {
    use maud::{Markup, html};

    use crate::{common, image::dto::ImageInfo};

    pub fn index(image: &str, info: &ImageInfo) -> Markup {
        html! {
            html {
                (common::view::head())
                body {
                    h1 { (image) }
                    table {
                        thead {
                            tr {
                                th { "Tag" }
                                th { "Digest" }
                                th { "" }
                            }
                        }
                        tbody {
                            @for tag in &info.tags {
                                tr {
                                    td { (tag.name) }
                                    td { (tag.digest) }
                                    td { button { "Delete" } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
