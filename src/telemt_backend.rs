#[path = "telemt_backend/api_client.rs"]
mod api_client;
#[path = "telemt_backend/api_dto.rs"]
mod api_dto;
#[path = "telemt_backend/control_api.rs"]
mod control_api;
#[path = "telemt_backend/legacy.rs"]
mod legacy;
#[path = "telemt_backend/mappers.rs"]
mod mappers;
#[path = "telemt_backend/types.rs"]
mod types;

use std::sync::Arc;

use anyhow::anyhow;

use crate::config::TelemtApiConfig;
use crate::runtime::TelemtRuntime;
use crate::telemt_cfg::TelemtConfig;

use control_api::ApiTelemtBackend;
use legacy::LegacyTelemtBackend;

#[allow(unused_imports)]
pub use types::{
    DeleteUserResult, ProvisionedUser, TelemtApiError, TelemtBackendMode, TelemtConnectionTopUser,
    TelemtConnectionsSummary, TelemtMonitorSnapshot, TelemtRuntimeEvent, TelemtRuntimeSnapshot,
    TelemtStatsSummary, TelemtUpstream, TelemtUpstreamDc, TelemtUpstreamsSummary, TelemtUserInfo,
    TelemtUserPatch,
};

#[derive(Clone)]
pub struct TelemtBackend {
    inner: Arc<TelemtBackendInner>,
}

enum TelemtBackendInner {
    Legacy(LegacyTelemtBackend),
    Api(ApiTelemtBackend),
}

impl TelemtBackend {
    pub fn new(
        api_cfg: &TelemtApiConfig,
        telemt_cfg: Arc<TelemtConfig>,
        telemt_runtime: TelemtRuntime,
    ) -> Result<Self, anyhow::Error> {
        let inner = if api_cfg.enabled {
            TelemtBackendInner::Api(ApiTelemtBackend::new(api_cfg, telemt_cfg, telemt_runtime)?)
        } else {
            TelemtBackendInner::Legacy(LegacyTelemtBackend::new(telemt_cfg, telemt_runtime))
        };

        Ok(Self {
            inner: Arc::new(inner),
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
        expiration_days: Option<i32>,
        max_unique_ips: Option<i32>,
        data_quota_bytes: Option<i64>,
    ) -> Result<ProvisionedUser, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(legacy) => {
                legacy.provision_user(username, desired_secret).await
            }
            TelemtBackendInner::Api(api) => {
                api.provision_user(
                    username,
                    desired_secret,
                    expiration_days,
                    max_unique_ips,
                    data_quota_bytes,
                )
                .await
            }
        }
    }

    pub async fn delete_user(&self, username: &str) -> Result<DeleteUserResult, anyhow::Error> {
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

    pub async fn get_user_info(
        &self,
        username: &str,
        cached_secret: Option<&str>,
    ) -> Result<Option<TelemtUserInfo>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(legacy) => legacy.get_user_info(cached_secret).await,
            TelemtBackendInner::Api(api) => api.get_user_info(username, cached_secret).await,
        }
    }

    pub async fn patch_user(
        &self,
        username: &str,
        patch: &TelemtUserPatch,
    ) -> Result<TelemtUserInfo, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Err(anyhow!(
                "Изменение лимитов пользователя доступно только через telemt control API"
            )),
            TelemtBackendInner::Api(api) => api.patch_user(username, patch).await,
        }
    }

    pub async fn stats_summary(&self) -> Result<Option<TelemtStatsSummary>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Ok(None),
            TelemtBackendInner::Api(api) => api.stats_summary().await.map(Some),
        }
    }

    pub async fn connections_summary(
        &self,
        limit: usize,
    ) -> Result<Option<TelemtConnectionsSummary>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Ok(None),
            TelemtBackendInner::Api(api) => api.connections_summary(limit).await,
        }
    }

    pub async fn monitor_snapshot(&self) -> Result<Option<TelemtMonitorSnapshot>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Ok(None),
            TelemtBackendInner::Api(api) => api.monitor_snapshot().await.map(Some),
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

    pub async fn rotate_secret(
        &self,
        username: &str,
    ) -> Result<ProvisionedUser, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Err(anyhow!(
                "Ротация секрета доступна только через telemt control API"
            )),
            TelemtBackendInner::Api(api) => api.rotate_secret(username).await,
        }
    }

    pub async fn upstreams_summary(
        &self,
    ) -> Result<Option<TelemtUpstreamsSummary>, anyhow::Error> {
        match self.inner.as_ref() {
            TelemtBackendInner::Legacy(_) => Ok(None),
            TelemtBackendInner::Api(api) => {
                let data = api.upstreams_summary().await?;
                if !data.enabled {
                    anyhow::bail!("Upstreams endpoint отклющён в настройках telemt");
                }
                let summary = match data.summary {
                    Some(s) => s,
                    None => return Ok(None),
                };
                let upstreams = data
                    .upstreams
                    .unwrap_or_default()
                    .into_iter()
                    .map(|u| {
                        let dc_list = u
                            .dc
                            .into_iter()
                            .map(|d| TelemtUpstreamDc {
                                dc: d.dc,
                                latency_ms: d.latency_ema_ms,
                                healthy: d.latency_ema_ms.is_some(),
                            })
                            .collect();
                        TelemtUpstream {
                            route_kind: u.route_kind,
                            healthy: u.healthy,
                            last_check_age_secs: u.last_check_age_secs,
                            dc_list,
                        }
                    })
                    .collect();
                let route_kinds = {
                    let mut kinds = Vec::new();
                    if summary.direct_total > 0 {
                        kinds.push(format!("Direct:{}", summary.direct_total));
                    }
                    if summary.socks5_total > 0 {
                        kinds.push(format!("Socks5:{}", summary.socks5_total));
                    }
                    if summary.shadowsocks_total > 0 {
                        kinds.push(format!("Shadowsocks:{}", summary.shadowsocks_total));
                    }
                    if summary.socks4_total > 0 {
                        kinds.push(format!("Socks4:{}", summary.socks4_total));
                    }
                    kinds.join(" | ")
                };
                Ok(Some(TelemtUpstreamsSummary {
                    healthy_total: summary.healthy_total,
                    configured_total: summary.configured_total,
                    unhealthy_total: summary.unhealthy_total,
                    route_kinds,
                    upstreams,
                }))
            }
        }
    }
}
