pub mod dto {
    use chrono::Utc;

    use crate::common::service::Paginated;

    pub struct ImageInfo {
        pub tags: Paginated<Tag>,
    }

    pub struct Tag {
        pub name: String,
        pub digest: String,
        pub error: bool,
        pub architecture: Option<String>,
        pub created: Option<chrono::DateTime<Utc>>,
        pub created_since: Option<chrono::Duration>,
    }
}

pub mod handler {
    use axum::{
        extract::{Path, Query, State},
        response::Redirect,
    };
    use joy_error::log::ResultLogExt;
    use maud::Markup;

    use crate::{
        AppState,
        auth::middleware::Authenticated,
        common::handler::PaginationQuery,
        image::{service, view},
    };

    pub async fn index(
        Path(image_name): Path<String>,
        Query(pagination): Query<PaginationQuery>,
        State(AppState {
            registry_api_client,
            ..
        }): State<AppState>,
        _: Authenticated,
    ) -> Result<Markup, Redirect> {
        service::get_image_info(registry_api_client, &image_name, pagination)
            .await
            .error()
            .log_err()
            .map_or_else(
                |_| Err(Redirect::to("/")),
                |info| Ok(view::index(&image_name, &info)),
            )
    }

    pub async fn delete_tag(
        Path((image_name, digest)): Path<(String, String)>,
        State(AppState {
            registry_api_client,
            ..
        }): State<AppState>,
    ) -> Redirect {
        if service::delete_tag(&registry_api_client, &image_name, &digest)
            .await
            .is_err()
        {
            tracing::error!("Could not delete image tag {image_name}:{digest}");
        }
        Redirect::to(&format!("/{image_name}"))
    }
}

pub mod service {

    use joy_error::log::ResultLogExt;

    use crate::{
        common::handler::PaginationQuery,
        error::service::ServiceResult,
        image::dto::{ImageInfo, Tag},
        registry,
    };

    pub async fn delete_tag(
        registry_api_client: &registry::api::Client,
        image_name: &str,
        digest: &str,
    ) -> ServiceResult<()> {
        registry_api_client
            .delete_tag(image_name, digest)
            .await
            .error()
            .log_err()?;
        Ok(())
    }

    pub async fn get_image_tags(
        registry_api_client: &registry::api::Client,
        image_name: &str,
    ) -> ServiceResult<Vec<String>> {
        Ok(registry_api_client
            .tags(image_name)
            .await
            .error()
            .log_err()?
            .tags
            .unwrap_or_default())
    }

    pub async fn get_image_info(
        registry_api_client: registry::api::Client,
        image_name: &str,
        pagination: PaginationQuery,
    ) -> ServiceResult<ImageInfo> {
        let tags = get_image_tags(&registry_api_client, image_name).await?;
        let tags = pagination.into_paginated(15, &tags)?;
        let mut tags = tags
            .map(|tag| async {
                let digest_response = registry_api_client.manifest(image_name, &tag).await?;
                let tag = match digest_response {
                    registry::dto::TagManifest::Nominal {
                        digest,
                        created,
                        architecture,
                    } => Tag {
                        digest,
                        created: Some(created),
                        created_since: Some(chrono::Utc::now() - created),
                        architecture: Some(architecture),
                        error: false,
                        name: tag,
                    },
                    registry::dto::TagManifest::MultiArch {
                        digest,
                        architectures,
                        created,
                    } => Tag {
                        digest,
                        created,
                        created_since: created.map(|c| chrono::Utc::now() - c),
                        architecture: Some(architectures.join(", ")),
                        error: false,
                        name: tag,
                    },
                    registry::dto::TagManifest::Error { digest } => Tag {
                        digest,
                        created: None,
                        created_since: None,
                        architecture: None,
                        error: true,
                        name: tag,
                    },
                };
                anyhow::Ok(tag)
            })
            .into_future()
            .await
            .into_result()?;
        tags.data.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(ImageInfo { tags })
    }
}

pub mod view {
    use maud::{Markup, html};

    use crate::{
        common::{self, service::Paginated},
        image::dto::ImageInfo,
    };

    pub fn index(image_name: &str, info: &ImageInfo) -> Markup {
        const LEFT_ARROW: &str = "\u{1F850}";
        const CLIPBOARD: &str = "\u{1F4CB}";

        let registry_host = common::service::env::registry_host();

        common::view::page().js("clipboard").content(html! {
            .d-flex .justify-content-between .m-2 {
                .d-flex .align-items-center .gap-3 {
                    a .text-decoration-none href="/" { .fs-1 { (LEFT_ARROW) } }
                    h1 { (image_name) " image tags" }
                }
                @if !info.tags.is_empty() && info.tags.need_pagination() {
                    .d-flex .justify-content-end {
                        (pagination_fragment(&info.tags, &format!("/{image_name}")))
                    }
                }
            }

            @if info.tags.is_empty() {
                p { "No tags found." }
            } @else {

                table .table .table-striped .table-bordered .table-hover .table-responsive .m-0 .align-middle .text-center {
                    thead {
                        tr {
                            th { "Creation Date" }
                            th { "Tag" }
                            th { "Digest" }
                            th { "Architecture" }
                            th { "Action" }
                        }
                    }
                    tbody {
                        @for tag in info.tags.iter() {
                            tr {
                                td { (tag.created.map(|date| format!("{}", date.format("%Y-%m-%d %H:%M:%S"))).as_deref().unwrap_or("?")) " (" (tag.created_since.map(format_duration).as_deref().unwrap_or("?")) " ago)"}
                                td {
                                    .d-flex .align-items-center .justify-content-between {
                                        (tag.name)
                                        .copy-button role="button" onclick="copyToClipboard(this)" data-image=(format!("{registry_host}/{image_name}:{}", tag.name)) { (CLIPBOARD) }
                                    }
                                }
                                td .text-danger[tag.error] { (tag.digest) }
                                td { (tag.architecture.as_deref().unwrap_or("?")) }
                                td {
                                    form .m-0 method="post" action=(format!("/{image_name}/delete/{}", tag.digest)) {
                                        button .btn .btn-danger type="submit" { "Delete" }
                                    }
                                }
                            }
                        }
                    }
                }
                @if info.tags.need_pagination() {
                    .d-flex .justify-content-end .mx-2 {
                        (pagination_fragment(&info.tags, &format!("/{image_name}")))
                    }
                }
            }
        }).call()
    }

    fn pagination_fragment<T>(pagination: &Paginated<T>, prefix: &str) -> Markup {
        let page = pagination.page;
        let size = pagination.size;
        let total_pages = pagination.total_pages();

        html! {
            .d-flex .my-2 .gap-2 {
                @if page > 0 {
                    a .btn .btn-primary href=(format!("{prefix}?page={}&size={}", pagination.previous(), size)) { "Previous" }
                }
                span .align-self-center { (page + 1) " / " (total_pages + 1) }
                @if page < total_pages {
                    a .btn .btn-primary href=(format!("{prefix}?page={}&size={}", pagination.next(), size)) { "Next" }
                }
            }
        }
    }

    fn format_duration(duration: chrono::Duration) -> String {
        if duration.num_hours() > 23 {
            format!("{} day(s)", duration.num_days())
        } else if duration.num_minutes() > 59 {
            format!("{} hour(s)", duration.num_hours())
        } else if duration.num_seconds() > 59 {
            format!("{} minute(s)", duration.num_minutes())
        } else {
            format!("{} second(s)", duration.num_seconds())
        }
    }
}
