pub mod handler {
    use crate::common::service::Paginated;
    use anyhow::ensure;
    use axum::response::IntoResponse;
    use serde::Deserialize;

    #[derive(Clone, Copy, Deserialize)]
    pub struct PaginationQuery {
        pub page: Option<usize>,
        pub size: Option<usize>,
    }

    impl PaginationQuery {
        pub fn into_paginated<T: Clone>(
            self,
            default_page_size: usize,
            data: &[T],
        ) -> anyhow::Result<Paginated<T>> {
            let size = self.size.unwrap_or(default_page_size);
            ensure!(size > 0);
            let page = self.page.unwrap_or(0);
            let total_element_count = data.len();

            let start = page * size;
            let end = (start + size).min(data.len());
            ensure!(end <= data.len());
            ensure!(start < end);
            let data = data[start..end].to_vec();
            Ok(Paginated {
                page,
                size,
                total_element_count,
                data,
            })
        }
    }

    pub async fn health() -> impl IntoResponse {
        "OK"
    }
}

pub mod service {
    pub const APP_VERSION: &str = const {
        if cfg!(debug_assertions) {
            concat!("dev build based on v", env!("CARGO_PKG_VERSION"))
        } else {
            concat!("v", env!("CARGO_PKG_VERSION"))
        }
    };

    /// Pagination struct
    ///
    /// `page` is 0 indexed
    /// `data` only contains the data for the current `page`
    pub struct Paginated<T> {
        pub page: usize,
        pub size: usize,
        pub total_element_count: usize,
        pub data: Vec<T>,
    }

    impl<T> Paginated<T> {
        pub const fn previous(&self) -> usize {
            self.page.saturating_sub(1)
        }

        pub const fn total_pages(&self) -> usize {
            self.total_element_count.div_ceil(self.size)
        }

        pub const fn next(&self) -> usize {
            if self.page + 1 < self.total_pages() {
                self.page + 1
            } else {
                self.page
            }
        }

        pub const fn need_pagination(&self) -> bool {
            self.total_pages() > 1
        }

        pub const fn is_empty(&self) -> bool {
            self.data.is_empty()
        }

        pub fn map<U, F>(self, f: F) -> Paginated<U>
        where
            F: FnMut(T) -> U,
        {
            Paginated {
                page: self.page,
                size: self.size,
                total_element_count: self.total_element_count,
                data: self.data.into_iter().map(f).collect(),
            }
        }

        pub fn iter(&self) -> impl Iterator<Item = &T> {
            self.data.iter()
        }
    }

    impl<T: Future> Paginated<T> {
        pub async fn into_future(self) -> Paginated<T::Output> {
            Paginated {
                page: self.page,
                size: self.size,
                total_element_count: self.total_element_count,
                data: futures::future::join_all(self.data.into_iter()).await,
            }
        }
    }

    impl<T, E> Paginated<Result<T, E>> {
        pub fn into_result(self) -> Result<Paginated<T>, E> {
            let paginated_elements = self.data.into_iter().collect::<Result<Vec<_>, _>>()?;
            Ok(Paginated {
                page: self.page,
                size: self.size,
                total_element_count: self.total_element_count,
                data: paginated_elements,
            })
        }
    }

    pub mod auth {}
}

pub mod view {
    use maud::{Markup, html};

    use crate::common::service;

    pub fn head() -> Markup {
        html! {
            head {
                title { "Docker Registry Explorer" }
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.8/dist/css/bootstrap.min.css" rel="stylesheet" integrity="sha384-sRIl4kxILFvY47J16cr9ZwB07vP4J8+LH7qKQnuqkuIAvNWLzeN8tE5YBujZqJLB" crossorigin="anonymous";
                link rel="stylesheet" href="/static/css/main.css";
            }
        }
    }

    pub fn header() -> Markup {
        html! {
            header .d-flex .justify-content-between .align-items-center .py-2 .px-2 {
                h1 .m-0 { "Docker Registry Explorer" }
                form .m-0 method="post" action="/auth/logout" {
                     button .btn .btn-primary type="submit" { "Logout" }
                }
            }
        }
    }

    pub fn footer() -> Markup {
        html! {
            footer .d-flex .justify-content-center .align-items-center .py-2 .px-2 .mx-2 .border-top {
                p .m-0 { "Docker Registry Explorer \u{B7} " (format!("{}", service::APP_VERSION)) }
            }
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn page(content: Markup) -> Markup {
        html! {
            html {
                (head())
            }
            body .d-flex .flex-column .min-vh-100 {
                (header())
                main .flex-fill {
                    (content)
                }
                (footer())
            }
        }
    }
}
