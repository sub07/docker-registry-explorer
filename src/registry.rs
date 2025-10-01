pub mod api {
    use std::borrow::ToOwned;

    use anyhow::anyhow;
    use serde::de::DeserializeOwned;

    use crate::{
        common,
        registry::dto::{CatalogResponse, ManifestBlob, TagManifest, TagsResponse},
    };

    #[derive(Clone, Debug)]
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
                    common::service::APP_VERSION
                ))
                .build()?;

            Ok(Self {
                inner: client,
                base_url: format!("{registry_url}/v2"),
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
                .request(method, format!("{}/{path}", self.base_url))
                .header("accept", "application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json")
                .basic_auth(self.username.clone(), Some(self.password.clone()))
                .send()
                .await?
                .json()
                .await?)
        }

        pub async fn catalog(&self) -> anyhow::Result<CatalogResponse> {
            self.make_request(reqwest::Method::GET, "_catalog").await
        }

        pub async fn count_tags(&self, image: &str) -> anyhow::Result<usize> {
            let tags = self.tags(image).await?;
            Ok(tags.tags.map_or(0, |tags| tags.len()))
        }

        pub async fn tags(&self, image: &str) -> anyhow::Result<TagsResponse> {
            self.make_request(reqwest::Method::GET, &format!("{image}/tags/list"))
                .await
        }

        pub async fn digest(&self, image: &str, tag: &str) -> anyhow::Result<TagManifest> {
            let response = self
                .inner
                .get(format!("{}/{image}/manifests/{tag}", self.base_url))
                .basic_auth(self.username.clone(), Some(self.password.clone()))
                .header("accept", "application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json")
                .send()
                .await?;
            let header_digest = response
                .headers()
                .get("docker-content-digest")
                .ok_or_else(|| anyhow!("docker-content-digest is missing from response"))
                .and_then(|header| header.to_str().map_err(|err| anyhow!(err)))
                .map(ToOwned::to_owned);
            let json = response.json::<serde_json::Value>().await?;
            if let Ok(digest) = header_digest {
                let config_digest = json
                    .get("config")
                    .ok_or_else(|| anyhow!("config missing"))?
                    .get("digest")
                    .ok_or_else(|| anyhow!("digest missing"))?
                    .as_str()
                    .ok_or_else(|| anyhow!("not a string"))?
                    .to_owned();
                let blob = self
                    .inner
                    .get(format!("{}/{image}/blobs/{config_digest}", self.base_url))
                    .header("accept", "application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json")
                    .basic_auth(self.username.clone(), Some(self.password.clone()))
                    .send()
                    .await?.json::<ManifestBlob>().await?;
                let created = chrono::DateTime::parse_from_rfc3339(&blob.created)?.to_utc();
                Ok(TagManifest::Nominal {
                    digest,
                    created,
                    architecture: blob.architecture,
                })
            } else {
                Ok(TagManifest::Error {
                    digest: json
                        .get("errors")
                        .ok_or_else(|| anyhow!("errors missing"))?
                        .as_array()
                        .ok_or_else(|| anyhow!("not an array"))?
                        .first()
                        .ok_or_else(|| anyhow!("empty array"))?
                        .get("detail")
                        .ok_or_else(|| anyhow!("detail missing"))?
                        .get("Revision")
                        .ok_or_else(|| anyhow!("revision missing"))?
                        .as_str()
                        .ok_or_else(|| anyhow!("not a string"))?
                        .to_owned(),
                })
            }
        }

        pub async fn delete_tag(&self, image: &str, digest: &str) -> anyhow::Result<()> {
            self.inner
                .delete(format!("{}/{image}/manifests/{digest}", self.base_url))
                .basic_auth(self.username.clone(), Some(self.password.clone()))
                .send()
                .await?
                .error_for_status()?;

            Ok(())
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
        pub tags: Option<Vec<String>>,
    }

    pub enum TagManifest {
        Nominal {
            digest: String,
            created: chrono::DateTime<chrono::Utc>,
            architecture: String,
        },
        Error {
            digest: String,
        },
    }

    #[derive(Deserialize)]
    pub struct ManifestBlob {
        pub architecture: String,
        pub created: String,
    }
}
