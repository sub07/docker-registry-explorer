pub mod api {
    use std::borrow::ToOwned;

    use anyhow::anyhow;
    use serde::de::DeserializeOwned;
    use tracing::{info, instrument};

    use crate::{
        common,
        registry::dto::{
            CatalogResponse, ManifestBlob, ManifestListResponse, TagManifest, TagsResponse,
        },
    };

    #[derive(Clone, Debug)]
    pub struct Client {
        inner: reqwest::Client,
        base_url: String,
        username: &'static str,
        password: &'static str,
    }

    impl Client {
        pub fn new(
            registry_host: &str,
            username: &'static str,
            password: &'static str,
        ) -> anyhow::Result<Self> {
            let client = reqwest::Client::builder()
                .user_agent(format!(
                    "Docker Registry Explorer v{}",
                    common::service::APP_VERSION
                ))
                .build()?;

            Ok(Self {
                inner: client,
                base_url: format!("https://{registry_host}/v2"),
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
                .basic_auth(self.username, Some(self.password))
                .send()
                .await?
                .json()
                .await?)
        }

        pub async fn catalog(&self) -> anyhow::Result<CatalogResponse> {
            self.make_request(reqwest::Method::GET, "_catalog").await
        }

        #[instrument(skip(self))]
        pub async fn count_tags(&self, image: &str) -> anyhow::Result<usize> {
            let tags = self.tags(image).await?;
            Ok(tags.tags.map_or(0, |tags| tags.len()))
        }

        #[instrument(skip(self))]
        pub async fn tags(&self, image: &str) -> anyhow::Result<TagsResponse> {
            self.make_request(reqwest::Method::GET, &format!("{image}/tags/list"))
                .await
        }

        #[instrument(skip(self))]
        pub async fn manifest(&self, image: &str, tag: &str) -> anyhow::Result<TagManifest> {
            let response = self
                .inner
                .get(format!("{}/{image}/manifests/{tag}", self.base_url))
                .basic_auth(self.username, Some(self.password))
                .header("accept", "application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json")
                .send()
                .await?;

            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_owned();

            let header_digest = response
                .headers()
                .get("docker-content-digest")
                .ok_or_else(|| anyhow!("docker-content-digest is missing from response"))
                .and_then(|header| header.to_str().map_err(|err| anyhow!(err)))
                .map(ToOwned::to_owned);

            let is_multi_arch = content_type.contains("manifest.list")
                || content_type.contains("image.index");

            if is_multi_arch {
                self.handle_multi_arch_manifest(image, header_digest, response)
                    .await
            } else {
                self.handle_single_manifest(image, header_digest, response)
                    .await
            }
        }

        async fn handle_single_manifest(
            &self,
            image: &str,
            header_digest: Result<String, anyhow::Error>,
            response: reqwest::Response,
        ) -> anyhow::Result<TagManifest> {
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
                    .basic_auth(self.username, Some(self.password))
                    .send()
                    .await?
                    .json::<ManifestBlob>()
                    .await?;
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

        async fn handle_multi_arch_manifest(
            &self,
            image: &str,
            header_digest: Result<String, anyhow::Error>,
            response: reqwest::Response,
        ) -> anyhow::Result<TagManifest> {
            let digest = header_digest?;
            let manifest_list = response.json::<ManifestListResponse>().await?;

            if manifest_list.manifests.is_empty() {
                return Ok(TagManifest::Error { digest });
            }

            let architectures: Vec<String> = manifest_list
                .manifests
                .iter()
                .filter_map(|entry| {
                    entry.platform.as_ref().and_then(|p| {
                        if p.os == "unknown" && p.architecture == "unknown" {
                            return None;
                        }
                        let base = format!("{}/{}", p.os, p.architecture);
                        Some(match &p.variant {
                            Some(v) => format!("{base}/{v}"),
                            None => base,
                        })
                    })
                })
                .collect();

            // Find linux/amd64 entry, or fall back to first entry with a platform
            let preferred = manifest_list
                .manifests
                .iter()
                .find(|e| {
                    e.platform
                        .as_ref()
                        .is_some_and(|p| p.os == "linux" && p.architecture == "amd64")
                })
                .or_else(|| {
                    manifest_list
                        .manifests
                        .iter()
                        .find(|e| e.platform.is_some())
                })
                .unwrap_or(&manifest_list.manifests[0]);

            let created = self
                .fetch_created_date(image, &preferred.digest)
                .await
                .ok();

            Ok(TagManifest::MultiArch {
                digest,
                architectures,
                created,
            })
        }

        async fn fetch_created_date(
            &self,
            image: &str,
            manifest_digest: &str,
        ) -> anyhow::Result<chrono::DateTime<chrono::Utc>> {
            let manifest_response = self
                .inner
                .get(format!(
                    "{}/{image}/manifests/{manifest_digest}",
                    self.base_url
                ))
                .basic_auth(self.username, Some(self.password))
                .header("accept", "application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json")
                .send()
                .await?;
            let json = manifest_response.json::<serde_json::Value>().await?;
            let config_digest = json
                .get("config")
                .ok_or_else(|| anyhow!("config missing"))?
                .get("digest")
                .ok_or_else(|| anyhow!("digest missing"))?
                .as_str()
                .ok_or_else(|| anyhow!("not a string"))?;
            let blob = self
                .inner
                .get(format!("{}/{image}/blobs/{config_digest}", self.base_url))
                .basic_auth(self.username, Some(self.password))
                .send()
                .await?
                .json::<ManifestBlob>()
                .await?;
            let created = chrono::DateTime::parse_from_rfc3339(&blob.created)?.to_utc();
            Ok(created)
        }

        #[instrument(skip(self))]
        pub async fn delete_tag(&self, image: &str, digest: &str) -> anyhow::Result<()> {
            info!("Calling delete tag request");
            self.inner
                .delete(format!("{}/{image}/manifests/{digest}", self.base_url))
                .basic_auth(self.username, Some(self.password))
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
        MultiArch {
            digest: String,
            architectures: Vec<String>,
            created: Option<chrono::DateTime<chrono::Utc>>,
        },
        Error {
            digest: String,
        },
    }

    impl TagManifest {
        pub fn digest(&self) -> &str {
            match self {
                Self::Nominal { digest, .. }
                | Self::MultiArch { digest, .. }
                | Self::Error { digest } => digest,
            }
        }
    }

    #[derive(Deserialize)]
    pub struct ManifestBlob {
        pub architecture: String,
        pub created: String,
    }

    #[derive(Deserialize)]
    pub struct ManifestListResponse {
        pub manifests: Vec<ManifestPlatformEntry>,
    }

    #[derive(Deserialize)]
    pub struct ManifestPlatformEntry {
        pub digest: String,
        pub platform: Option<Platform>,
    }

    #[derive(Deserialize)]
    pub struct Platform {
        pub architecture: String,
        pub os: String,
        pub variant: Option<String>,
    }
}
