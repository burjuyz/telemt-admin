use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use reqwest::header::{AUTHORIZATION, HeaderValue, IF_MATCH};
use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::config::TelemtApiConfig;
use crate::link::build_proxy_link;
use crate::service::ServiceController;
use crate::telemt_cfg::TelemtConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemtBackendMode {
    LegacyFile,
    ControlApi,
}

impl TelemtBackendMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LegacyFile => "legacy_file",
            Self::ControlApi => "control_api",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProvisionedUser {
    pub secret: String,
    pub link: Option<String>,
    pub mode: TelemtBackendMode,
    pub revision: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TelemtRuntimeEvent {
    pub ts_epoch_secs: i64,
    pub event_type: String,
    pub context: String,
}

#[derive(Debug, Clone)]
pub struct TelemtRuntimeSnapshot {
    pub source: TelemtBackendMode,
    pub health_status: String,
    pub api_read_only: bool,
    pub build_version: Option<String>,
    pub transport_mode: Option<String>,
    pub startup_status: Option<String>,
    pub startup_stage: Option<String>,
    pub startup_progress_pct: Option<f64>,
    pub api_whitelist_enabled: Option<bool>,
    pub api_whitelist_entries: Option<usize>,
    pub api_auth_header_enabled: Option<bool>,
    /// `/v1/runtime/gates`
    pub accepting_new_connections: Option<bool>,
    pub me_runtime_ready: Option<bool>,
    pub use_middle_proxy: Option<bool>,
    pub route_mode: Option<String>,
    /// `/v1/runtime/me-selftest` — состояния KDF и рассинхрона времени
    pub me_selftest_kdf_state: Option<String>,
    pub me_selftest_timeskew_state: Option<String>,
    pub me_selftest_enabled: Option<bool>,
    /// `/v1/runtime/upstream_quality`
    pub upstream_configured_total: Option<u64>,
    pub upstream_healthy_total: Option<u64>,
    pub upstream_unhealthy_total: Option<u64>,
    pub events: Vec<TelemtRuntimeEvent>,
    pub last_revision: Option<String>,
}

#[derive(Clone)]
pub struct TelemtBackend {
    inner: Arc<TelemtBackendInner>,
}

enum TelemtBackendInner {
    Legacy(LegacyTelemtBackend),
    Api(ApiTelemtBackend),
}

#[derive(Clone)]
struct LegacyTelemtBackend {
    telemt_cfg: Arc<TelemtConfig>,
    service: ServiceController,
}

struct ApiTelemtBackend {
    client: Client,
    base_url: String,
    auth_header: Option<HeaderValue>,
    allow_file_fallback: bool,
    prefer_api_links: bool,
    telemt_cfg: Arc<TelemtConfig>,
    legacy_fallback: Option<LegacyTelemtBackend>,
    revision: Mutex<Option<String>>,
}

impl Clone for ApiTelemtBackend {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            base_url: self.base_url.clone(),
            auth_header: self.auth_header.clone(),
            allow_file_fallback: self.allow_file_fallback,
            prefer_api_links: self.prefer_api_links,
            telemt_cfg: self.telemt_cfg.clone(),
            legacy_fallback: self.legacy_fallback.clone(),
            revision: Mutex::new(None),
        }
    }
}

impl TelemtBackend {
    pub fn new(
        api_cfg: &TelemtApiConfig,
        telemt_cfg: Arc<TelemtConfig>,
        service: ServiceController,
    ) -> Result<Self, anyhow::Error> {
        if api_cfg.enabled {
            let legacy_fallback = api_cfg.allow_file_fallback.then(|| LegacyTelemtBackend {
                telemt_cfg: telemt_cfg.clone(),
                service: service.clone(),
            });
            let timeout = Duration::from_millis(api_cfg.timeout_ms.max(1));
            let client = Client::builder()
                .timeout(timeout)
                .build()
                .context("Не удалось создать HTTP-клиент telemt API")?;
            let auth_header = api_cfg
                .auth_header
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(HeaderValue::from_str)
                .transpose()
                .context("telemt_api.auth_header содержит невалидные символы")?;
            return Ok(Self {
                inner: Arc::new(TelemtBackendInner::Api(ApiTelemtBackend {
                    client,
                    base_url: api_cfg.base_url.trim_end_matches('/').to_string(),
                    auth_header,
                    allow_file_fallback: api_cfg.allow_file_fallback,
                    prefer_api_links: api_cfg.prefer_api_links,
                    telemt_cfg,
                    legacy_fallback,
                    revision: Mutex::new(None),
                })),
            });
        }

        Ok(Self {
            inner: Arc::new(TelemtBackendInner::Legacy(LegacyTelemtBackend {
                telemt_cfg,
                service,
            })),
        })
    }

    pub fn mode(&self) -> TelemtBackendMode {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => TelemtBackendMode::LegacyFile,
            TelemtBackendInner::Api(_) => TelemtBackendMode::ControlApi,
        }
    }

    pub async fn provision_user(
        &self,
        username: &str,
        desired_secret: &str,
    ) -> Result<ProvisionedUser, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(legacy) => {
                legacy.provision_user(username, desired_secret).await
            }
            TelemtBackendInner::Api(api) => api.provision_user(username, desired_secret).await,
        }
    }

    pub async fn delete_user(&self, username: &str) -> Result<bool, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(legacy) => legacy.delete_user(username).await,
            TelemtBackendInner::Api(api) => api.delete_user(username).await,
        }
    }

    pub async fn build_user_link(
        &self,
        username: &str,
        cached_secret: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(legacy) => legacy.build_user_link(cached_secret).await,
            TelemtBackendInner::Api(api) => api.build_user_link(username, cached_secret).await,
        }
    }

    pub async fn runtime_snapshot(
        &self,
        recent_events_limit: usize,
    ) -> Result<Option<TelemtRuntimeSnapshot>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Ok(None),
            TelemtBackendInner::Api(api) => match api.runtime_snapshot(recent_events_limit).await {
                Ok(snapshot) => Ok(Some(snapshot)),
                Err(error) => {
                    tracing::warn!(error = %error, "Не удалось получить runtime snapshot telemt API");
                    Ok(None)
                }
            },
        }
    }
}

impl LegacyTelemtBackend {
    async fn provision_user(
        &self,
        username: &str,
        desired_secret: &str,
    ) -> Result<ProvisionedUser, anyhow::Error> {
        self.telemt_cfg.upsert_user(username, desired_secret)?;
        let reload = self.service.notify_config_reloaded().await;
        if !reload.success {
            tracing::warn!(stderr = %reload.stderr, "telemt config reload/restart had issues");
        }
        let link = self.build_user_link(Some(desired_secret)).await?;
        Ok(ProvisionedUser {
            secret: desired_secret.to_string(),
            link: Some(link),
            mode: TelemtBackendMode::LegacyFile,
            revision: None,
        })
    }

    async fn delete_user(&self, username: &str) -> Result<bool, anyhow::Error> {
        let removed = self.telemt_cfg.remove_user(username)?;
        if removed {
            let reload = self.service.notify_config_reloaded().await;
            if !reload.success {
                tracing::warn!(stderr = %reload.stderr, "telemt config reload/restart had issues");
            }
        }
        Ok(removed)
    }

    async fn build_user_link(&self, cached_secret: Option<&str>) -> Result<String, anyhow::Error> {
        let secret =
            cached_secret.ok_or_else(|| anyhow!("Не найден локальный секрет пользователя"))?;
        let params = self.telemt_cfg.read_link_params()?;
        build_proxy_link(&params, secret).map_err(anyhow::Error::from)
    }
}

impl ApiTelemtBackend {
    async fn provision_user(
        &self,
        username: &str,
        desired_secret: &str,
    ) -> Result<ProvisionedUser, anyhow::Error> {
        let body = CreateUserRequest {
            username,
            secret: Some(desired_secret),
        };
        let response = self
            .mutate_with_retry::<CreateUserRequest, CreateUserResponse>(
                Method::POST,
                "/v1/users",
                Some(&body),
            )
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                if let Some(legacy) = &self.legacy_fallback {
                    tracing::warn!(
                        username = username,
                        error = %error,
                        "telemt API provision failed; falling back to legacy backend"
                    );
                    return legacy.provision_user(username, desired_secret).await;
                }
                return Err(error);
            }
        };
        let CreateUserResponse { user, secret } = response.data;
        let link = if self.prefer_api_links {
            pick_best_link(&user.links).or_else(|| self.try_build_fallback_link(Some(&secret)).ok())
        } else {
            self.try_build_fallback_link(Some(&secret))
                .ok()
                .or_else(|| pick_best_link(&user.links))
        };
        Ok(ProvisionedUser {
            secret,
            link,
            mode: TelemtBackendMode::ControlApi,
            revision: Some(response.revision),
        })
    }

    async fn delete_user(&self, username: &str) -> Result<bool, anyhow::Error> {
        let path = format!("/v1/users/{}", username);
        match self
            .mutate_with_retry::<(), String>(Method::DELETE, &path, None)
            .await
        {
            Ok(_) => Ok(true),
            Err(error) => {
                if let Some(api_error) = error.downcast_ref::<TelemtApiError>()
                    && api_error.status == StatusCode::NOT_FOUND
                {
                    return Ok(false);
                }
                if let Some(legacy) = &self.legacy_fallback {
                    tracing::warn!(
                        username = username,
                        error = %error,
                        "telemt API delete failed; falling back to legacy backend"
                    );
                    return legacy.delete_user(username).await;
                }
                Err(error)
            }
        }
    }

    async fn build_user_link(
        &self,
        username: &str,
        cached_secret: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        if self.prefer_api_links {
            match self.fetch_user_link(username).await {
                Ok(Some(link)) => return Ok(link),
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        username = username,
                        error = %error,
                        "telemt API link lookup failed; trying fallback"
                    );
                }
            }
            return self.try_build_fallback_link(cached_secret);
        }

        match self.try_build_fallback_link(cached_secret) {
            Ok(link) => return Ok(link),
            Err(error) => {
                tracing::warn!(
                    username = username,
                    error = %error,
                    "Local link build failed; trying telemt API lookup"
                );
            }
        }
        self.fetch_user_link(username)
            .await?
            .ok_or_else(|| anyhow!("Не удалось получить ссылку пользователя из telemt API"))
    }

    async fn runtime_snapshot(
        &self,
        recent_events_limit: usize,
    ) -> Result<TelemtRuntimeSnapshot, anyhow::Error> {
        let health = self.get_success::<HealthData>("/v1/health").await?;
        let system_info = self
            .get_success::<SystemInfoData>("/v1/system/info")
            .await?;
        let runtime_init = self
            .get_success::<RuntimeInitializationData>("/v1/runtime/initialization")
            .await?;
        let security = self
            .get_success::<SecurityPostureData>("/v1/security/posture")
            .await?;
        let HealthData {
            status: health_status,
            read_only: api_read_only,
        } = health.data;
        let SystemInfoData {
            version: build_version,
        } = system_info.data;
        let RuntimeInitializationData {
            status: startup_status,
            current_stage: startup_stage,
            progress_pct: startup_progress_pct,
            transport_mode,
        } = runtime_init.data;
        let SecurityPostureData {
            api_whitelist_enabled,
            api_whitelist_entries,
            api_auth_header_enabled,
        } = security.data;
        let events_path = format!(
            "/v1/runtime/events/recent?limit={}",
            recent_events_limit.max(1)
        );
        let events = self
            .get_success::<RuntimeEventsData>(&events_path)
            .await
            .ok();

        let gates = match self.get_success::<RuntimeGatesData>("/v1/runtime/gates").await {
            Ok(envelope) => Some(envelope.data),
            Err(error) => {
                tracing::debug!(error = %error, "telemt API /v1/runtime/gates недоступен");
                None
            }
        };
        let me_selftest = match self
            .get_success::<MeSelftestTop>("/v1/runtime/me-selftest")
            .await
        {
            Ok(envelope) => Some(envelope.data),
            Err(error) => {
                tracing::debug!(error = %error, "telemt API /v1/runtime/me-selftest недоступен");
                None
            }
        };
        let upstream = match self
            .get_success::<UpstreamQualityTop>("/v1/runtime/upstream_quality")
            .await
        {
            Ok(envelope) => Some(envelope.data),
            Err(error) => {
                tracing::debug!(error = %error, "telemt API /v1/runtime/upstream_quality недоступен");
                None
            }
        };

        let (
            accepting_new_connections,
            me_runtime_ready,
            use_middle_proxy,
            route_mode,
        ) = match &gates {
            Some(g) => (
                Some(g.accepting_new_connections),
                Some(g.me_runtime_ready),
                Some(g.use_middle_proxy),
                Some(g.route_mode.clone()),
            ),
            None => (None, None, None, None),
        };

        let (me_selftest_enabled, me_selftest_kdf_state, me_selftest_timeskew_state) =
            match &me_selftest {
                Some(m) => {
                    let kdf = m
                        .data
                        .as_ref()
                        .map(|p| p.kdf.state.clone());
                    let timeskew = m
                        .data
                        .as_ref()
                        .map(|p| p.timeskew.state.clone());
                    (Some(m.enabled), kdf, timeskew)
                }
                None => (None, None, None),
            };

        let (upstream_configured_total, upstream_healthy_total, upstream_unhealthy_total) =
            match &upstream {
                Some(u) if u.enabled => match &u.summary {
                    Some(s) => (
                        Some(s.configured_total),
                        Some(s.healthy_total),
                        Some(s.unhealthy_total),
                    ),
                    None => (None, None, None),
                },
                _ => (None, None, None),
            };

        let event_rows = events
            .map(|payload| {
                payload
                    .data
                    .data
                    .map(|payload| {
                        payload
                            .events
                            .into_iter()
                            .map(|event| TelemtRuntimeEvent {
                                ts_epoch_secs: event.ts_epoch_secs,
                                event_type: event.event_type,
                                context: event.context,
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let revision = self.revision.lock().await.clone();

        Ok(TelemtRuntimeSnapshot {
            source: TelemtBackendMode::ControlApi,
            health_status,
            api_read_only,
            build_version: Some(build_version),
            transport_mode: Some(transport_mode),
            startup_status: Some(startup_status),
            startup_stage: Some(startup_stage),
            startup_progress_pct: Some(startup_progress_pct),
            api_whitelist_enabled: Some(api_whitelist_enabled),
            api_whitelist_entries: Some(api_whitelist_entries),
            api_auth_header_enabled: Some(api_auth_header_enabled),
            accepting_new_connections,
            me_runtime_ready,
            use_middle_proxy,
            route_mode,
            me_selftest_kdf_state,
            me_selftest_timeskew_state,
            me_selftest_enabled,
            upstream_configured_total,
            upstream_healthy_total,
            upstream_unhealthy_total,
            events: event_rows,
            last_revision: revision,
        })
    }

    async fn fetch_user_link(&self, username: &str) -> Result<Option<String>, anyhow::Error> {
        let path = format!("/v1/users/{}", username);
        let response = self.get_success::<UserInfo>(&path).await?;
        Ok(pick_best_link(&response.data.links))
    }

    fn try_build_fallback_link(
        &self,
        cached_secret: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        if !self.allow_file_fallback {
            return Err(anyhow!("Fallback на локальный telemt.toml отключён"));
        }
        let secret =
            cached_secret.ok_or_else(|| anyhow!("Не найден локальный секрет пользователя"))?;
        let params = self.telemt_cfg.read_link_params()?;
        build_proxy_link(&params, secret).map_err(anyhow::Error::from)
    }

    async fn get_success<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
    ) -> Result<SuccessEnvelope<T>, anyhow::Error> {
        let request = self
            .client
            .request(Method::GET, self.endpoint(path))
            .headers(self.base_headers()?);
        let response = request
            .send()
            .await
            .with_context(|| format!("Не удалось выполнить GET {}", path))?;
        self.parse_success_response(response).await
    }

    async fn mutate_with_retry<TBody: Serialize + ?Sized, TData: for<'de> Deserialize<'de>>(
        &self,
        method: Method,
        path: &str,
        body: Option<&TBody>,
    ) -> Result<SuccessEnvelope<TData>, anyhow::Error> {
        let mut retried = false;
        loop {
            if self.revision.lock().await.is_none() {
                let revision = self.refresh_revision().await?;
                *self.revision.lock().await = Some(revision);
            }
            let revision = self.revision.lock().await.clone();
            let mut request = self
                .client
                .request(method.clone(), self.endpoint(path))
                .headers(self.base_headers()?);
            if let Some(revision) = revision.as_deref() {
                request = request.header(IF_MATCH, revision);
            }
            if let Some(body) = body {
                request = request.json(body);
            }
            let response = request
                .send()
                .await
                .with_context(|| format!("Не удалось выполнить {} {}", method, path))?;
            match self.parse_success_response(response).await {
                Ok(success) => return Ok(success),
                Err(error) => {
                    let needs_retry = error
                        .downcast_ref::<TelemtApiError>()
                        .map(|api_error| {
                            api_error.status == StatusCode::CONFLICT
                                && api_error.code == Some("revision_conflict".to_string())
                        })
                        .unwrap_or(false);
                    if needs_retry && !retried {
                        retried = true;
                        let revision = self.refresh_revision().await?;
                        *self.revision.lock().await = Some(revision);
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    async fn refresh_revision(&self) -> Result<String, anyhow::Error> {
        let health = self.get_success::<HealthData>("/v1/health").await?;
        Ok(health.revision)
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn base_headers(&self) -> Result<reqwest::header::HeaderMap, anyhow::Error> {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(value) = self.auth_header.clone() {
            headers.insert(AUTHORIZATION, value);
        }
        Ok(headers)
    }

    async fn parse_success_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> Result<SuccessEnvelope<T>, anyhow::Error> {
        let status = response.status();
        let body = response
            .bytes()
            .await
            .context("Не удалось прочитать ответ telemt API")?;
        if status.is_success() {
            let envelope: SuccessEnvelope<T> =
                serde_json::from_slice(&body).with_context(|| {
                    format!(
                        "Не удалось декодировать успешный ответ: {}",
                        String::from_utf8_lossy(&body)
                    )
                })?;
            *self.revision.lock().await = Some(envelope.revision.clone());
            return Ok(envelope);
        }

        let parsed_error = serde_json::from_slice::<ErrorEnvelope>(&body).ok();
        let message = parsed_error
            .as_ref()
            .map(|error| error.error.message.clone())
            .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
        let code = parsed_error.as_ref().map(|error| error.error.code.clone());
        Err(TelemtApiError {
            status,
            code,
            message,
        }
        .into())
    }
}

fn pick_best_link(links: &UserLinks) -> Option<String> {
    links
        .tls
        .first()
        .cloned()
        .or_else(|| links.secure.first().cloned())
        .or_else(|| links.classic.first().cloned())
}

#[derive(Debug, thiserror::Error)]
#[error("telemt API error ({status}): {message}")]
pub struct TelemtApiError {
    pub status: StatusCode,
    pub code: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct SuccessEnvelope<T> {
    data: T,
    revision: String,
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct CreateUserRequest<'a> {
    username: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct CreateUserResponse {
    user: UserInfo,
    secret: String,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    links: UserLinks,
}

#[derive(Debug, Deserialize)]
struct UserLinks {
    #[serde(default)]
    classic: Vec<String>,
    #[serde(default)]
    secure: Vec<String>,
    #[serde(default)]
    tls: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HealthData {
    status: String,
    read_only: bool,
}

#[derive(Debug, Deserialize)]
struct SystemInfoData {
    version: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeInitializationData {
    status: String,
    current_stage: String,
    progress_pct: f64,
    transport_mode: String,
}

#[derive(Debug, Deserialize)]
struct SecurityPostureData {
    api_whitelist_enabled: bool,
    api_whitelist_entries: usize,
    api_auth_header_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct RuntimeEventsData {
    #[serde(default)]
    data: Option<RuntimeEventsPayload>,
}

#[derive(Debug, Deserialize)]
struct RuntimeEventsPayload {
    #[serde(default)]
    events: Vec<ApiEventRecord>,
}

#[derive(Debug, Deserialize)]
struct ApiEventRecord {
    ts_epoch_secs: i64,
    event_type: String,
    context: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeGatesData {
    accepting_new_connections: bool,
    me_runtime_ready: bool,
    use_middle_proxy: bool,
    route_mode: String,
}

#[derive(Debug, Deserialize)]
struct MeSelftestTop {
    enabled: bool,
    #[serde(default)]
    data: Option<MeSelftestPayloadDe>,
}

#[derive(Debug, Deserialize)]
struct MeSelftestPayloadDe {
    kdf: MeSelftestKdfDe,
    timeskew: MeSelftestTimeskewDe,
}

#[derive(Debug, Deserialize)]
struct MeSelftestKdfDe {
    state: String,
}

#[derive(Debug, Deserialize)]
struct MeSelftestTimeskewDe {
    state: String,
}

#[derive(Debug, Deserialize)]
struct UpstreamQualityTop {
    enabled: bool,
    #[serde(default)]
    summary: Option<UpstreamQualitySummaryDe>,
}

#[derive(Debug, Deserialize)]
struct UpstreamQualitySummaryDe {
    configured_total: u64,
    healthy_total: u64,
    unhealthy_total: u64,
}
