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
    use std::sync::LazyLock;

    pub const APP_VERSION: &str = const {
        if cfg!(debug_assertions) {
            concat!("dev build based on v", env!("CARGO_PKG_VERSION"))
        } else {
            concat!("v", env!("CARGO_PKG_VERSION"))
        }
    };

    static REGISTRY_HOST: LazyLock<String> =
        LazyLock::new(|| std::env::var("REGISTRY_HOST").expect("REGISTRY_HOST"));

    static REGISTRY_USERNAME: LazyLock<String> =
        LazyLock::new(|| std::env::var("REGISTRY_USERNAME").expect("REGISTRY_USERNAME"));

    static REGISTRY_PASSWORD: LazyLock<String> =
        LazyLock::new(|| std::env::var("REGISTRY_PASSWORD").expect("REGISTRY_PASSWORD"));

    static LISTEN_ADDR: LazyLock<String> =
        LazyLock::new(|| std::env::var("LISTEN_ADDR").expect("LISTEN_ADDR"));

    static LISTEN_PORT: LazyLock<String> =
        LazyLock::new(|| std::env::var("LISTEN_PORT").expect("LISTEN_PORT"));

    static STATIC_DIR: LazyLock<String> =
        LazyLock::new(|| std::env::var("STATIC_DIR").expect("STATIC_DIR"));

    static EXPLORER_USERNAME: LazyLock<String> =
        LazyLock::new(|| std::env::var("EXPLORER_USERNAME").expect("EXPLORER_USERNAME"));

    static EXPLORER_PASSWORD: LazyLock<String> =
        LazyLock::new(|| std::env::var("EXPLORER_PASSWORD").expect("EXPLORER_PASSWORD"));

    pub mod env {
        use super::{
            EXPLORER_PASSWORD, EXPLORER_USERNAME, LISTEN_ADDR, LISTEN_PORT, REGISTRY_HOST,
            REGISTRY_PASSWORD, REGISTRY_USERNAME, STATIC_DIR,
        };

        pub fn registry_host() -> &'static str {
            &REGISTRY_HOST
        }

        pub fn registry_username() -> &'static str {
            &REGISTRY_USERNAME
        }

        pub fn registry_password() -> &'static str {
            &REGISTRY_PASSWORD
        }

        pub fn listen_addr() -> &'static str {
            &LISTEN_ADDR
        }

        pub fn listen_port() -> &'static str {
            &LISTEN_PORT
        }

        pub fn static_dir() -> &'static str {
            &STATIC_DIR
        }

        pub fn explorer_username() -> &'static str {
            &EXPLORER_USERNAME
        }

        pub fn explorer_password() -> &'static str {
            &EXPLORER_PASSWORD
        }

        pub fn check() {
            let _ = registry_host();
            let _ = registry_username();
            let _ = registry_password();
            let _ = listen_addr();
            let _ = listen_port();
            let _ = static_dir();
            let _ = explorer_username();
            let _ = explorer_password();
        }
    }

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

    pub fn head_with_extra(js: Vec<&'static str>, css: Vec<&'static str>) -> Markup {
        html! {
            head {
                title { "Docker Registry Explorer" }
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.8/dist/css/bootstrap.min.css" rel="stylesheet" integrity="sha384-sRIl4kxILFvY47J16cr9ZwB07vP4J8+LH7qKQnuqkuIAvNWLzeN8tE5YBujZqJLB" crossorigin="anonymous";
                link rel="stylesheet" href="/static/css/main.css";
                @for css in css {
                    link rel="stylesheet" href=(format!("/static/css/{css}.css"));
                }
                @for js in js {
                    script defer src=(format!("/static/js/{js}.js")) {}
                }
            }
        }
    }

    pub fn head() -> Markup {
        head_with_extra(vec![], vec![])
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

    impl<S: page_builder::State> PageBuilder<S> {
        pub fn js(mut self, value: &'static str) -> Self {
            self.js.push(value);
            self
        }

        #[allow(dead_code)]
        pub fn css(mut self, value: &'static str) -> Self {
            self.css.push(value);
            self
        }
    }

    #[bon::builder]
    #[allow(clippy::needless_pass_by_value)]
    pub fn page(
        #[builder(field)] js: Vec<&'static str>,
        #[builder(field)] css: Vec<&'static str>,
        content: Markup,
    ) -> Markup {
        html! {
            html {
                (head_with_extra(js, css))
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
