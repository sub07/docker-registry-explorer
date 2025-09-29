pub mod api {
    use anyhow::anyhow;
    use serde::de::DeserializeOwned;

    use crate::registry::dto::{CatalogResponse, TagsResponse};

    #[derive(Clone)]
    pub struct Client {
        inner: reqwest::Client,
        base_url: String,
        username: String,
        password: String,
    }

    impl Client {
        pub fn new(registry_url: &str, username: String, password: String) -> anyhow::Result<Self> {
            let client = reqwest::Client::builder()
                .user_agent(format!(
                    "Docker Registry Explorer v{}",
                    env!("CARGO_PKG_VERSION")
                ))
                .build()?;

            Ok(Self {
                inner: client,
                base_url: format!("{registry_url}/v2/"),
                username,
                password,
            })
        }

        async fn make_request<Response: DeserializeOwned>(
            &self,
            method: reqwest::Method,
            path: &str,
        ) -> anyhow::Result<Response> {
            Ok(self
                .inner
                .request(method, format!("{}{path}", self.base_url))
                .basic_auth(self.username.clone(), Some(self.password.clone()))
                .send()
                .await?
                .json()
                .await?)
        }

        pub async fn catalog(&self) -> anyhow::Result<CatalogResponse> {
            self.make_request(reqwest::Method::GET, "_catalog").await
        }

        pub async fn tags(&self, image: &str) -> anyhow::Result<TagsResponse> {
            self.make_request(reqwest::Method::GET, &format!("{image}/tags/list"))
                .await
        }

        pub async fn digest(&self, image: &str, tag: &str) -> anyhow::Result<String> {
            Ok(self
                .inner
                .get(format!("{}{image}/manifests/{tag}", self.base_url))
                .basic_auth(self.username.clone(), Some(self.password.clone()))
                .header("accept", "application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json")
                .send()
                .await?
                .headers()
                .get("docker-content-digest")
                .ok_or_else(|| anyhow!("docker-content-digest is missing from response"))?
                .to_str()?
                .to_owned())
        }
    }
}

pub mod dto {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct CatalogResponse {
        pub repositories: Vec<String>,
    }

    #[derive(Deserialize)]
    pub struct TagsResponse {
        pub name: String,
        pub tags: Vec<String>,
    }
}
