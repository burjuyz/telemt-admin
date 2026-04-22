use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct SuccessEnvelope<T> {
    pub(crate) data: T,
    pub(crate) revision: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ErrorEnvelope {
    pub(crate) error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiErrorBody {
    pub(crate) code: String,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateUserRequest<'a> {
    pub(crate) username: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) secret: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) expiration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_unique_ips: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data_quota_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) group_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateUserResponse {
    pub(crate) user: UserInfo,
    pub(crate) secret: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserInfo {
    pub(crate) links: UserLinks,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ApiUserInfo {
    #[serde(default)]
    pub(crate) username: Option<String>,
    #[serde(default)]
    pub(crate) in_runtime: Option<bool>,
    #[serde(default)]
    pub(crate) user_ad_tag: Option<String>,
    #[serde(default)]
    pub(crate) max_tcp_conns: Option<usize>,
    #[serde(default)]
    pub(crate) expiration_rfc3339: Option<String>,
    #[serde(default)]
    pub(crate) data_quota_bytes: Option<u64>,
    #[serde(default)]
    pub(crate) max_unique_ips: Option<usize>,
    #[serde(default)]
    pub(crate) current_connections: Option<u64>,
    #[serde(default)]
    pub(crate) active_unique_ips: Option<usize>,
    #[serde(default)]
    pub(crate) active_unique_ips_list: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) recent_unique_ips: Option<usize>,
    #[serde(default)]
    pub(crate) recent_unique_ips_list: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) total_octets: Option<u64>,
    #[serde(default)]
    pub(crate) links: Option<UserLinks>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserLinks {
    #[serde(default)]
    pub(crate) classic: Vec<String>,
    #[serde(default)]
    pub(crate) secure: Vec<String>,
    #[serde(default)]
    pub(crate) tls: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct HealthData {
    pub(crate) status: String,
    pub(crate) read_only: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StatsSummaryData {
    pub(crate) uptime_seconds: f64,
    pub(crate) connections_total: u64,
    pub(crate) connections_bad_total: u64,
    pub(crate) handshake_timeouts_total: u64,
    pub(crate) configured_users: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SystemInfoData {
    pub(crate) version: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeInitializationData {
    pub(crate) status: String,
    pub(crate) current_stage: String,
    pub(crate) progress_pct: f64,
    pub(crate) transport_mode: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SecurityPostureData {
    pub(crate) api_whitelist_enabled: bool,
    pub(crate) api_whitelist_entries: usize,
    pub(crate) api_auth_header_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeEventsData {
    #[serde(default)]
    pub(crate) data: Option<RuntimeEventsPayload>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeEventsPayload {
    #[serde(default)]
    pub(crate) events: Vec<ApiEventRecord>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiEventRecord {
    pub(crate) ts_epoch_secs: i64,
    pub(crate) event_type: String,
    pub(crate) context: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeGatesData {
    pub(crate) accepting_new_connections: bool,
    pub(crate) me_runtime_ready: bool,
    pub(crate) use_middle_proxy: bool,
    pub(crate) route_mode: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MeSelftestTop {
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) data: Option<MeSelftestPayloadDe>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MeSelftestPayloadDe {
    pub(crate) kdf: MeSelftestKdfDe,
    pub(crate) timeskew: MeSelftestTimeskewDe,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MeSelftestKdfDe {
    pub(crate) state: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MeSelftestTimeskewDe {
    pub(crate) state: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpstreamQualityTop {
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) summary: Option<UpstreamQualitySummaryDe>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpstreamQualitySummaryDe {
    pub(crate) configured_total: u64,
    pub(crate) healthy_total: u64,
    pub(crate) unhealthy_total: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeConnectionsSummaryTop {
    #[allow(dead_code)]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) data: Option<RuntimeConnectionsSummaryPayload>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeConnectionsSummaryPayload {
    pub(crate) totals: RuntimeConnectionsTotals,
    pub(crate) top: RuntimeConnectionsTop,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeConnectionsTotals {
    pub(crate) current_connections: u64,
    pub(crate) current_connections_me: u64,
    pub(crate) current_connections_direct: u64,
    pub(crate) active_users: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeConnectionsTop {
    #[serde(default)]
    pub(crate) by_connections: Vec<RuntimeConnectionUserData>,
    #[serde(default)]
    pub(crate) by_throughput: Vec<RuntimeConnectionUserData>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RuntimeConnectionUserData {
    pub(crate) username: String,
    pub(crate) current_connections: u64,
    pub(crate) total_octets: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpstreamsData {
    #[allow(dead_code)]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) summary: Option<UpstreamsSummary>,
    #[serde(default)]
    pub(crate) upstreams: Option<Vec<Upstream>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpstreamsSummary {
    pub(crate) configured_total: u64,
    pub(crate) healthy_total: u64,
    pub(crate) unhealthy_total: u64,
    pub(crate) direct_total: u64,
    pub(crate) socks4_total: u64,
    pub(crate) socks5_total: u64,
    pub(crate) shadowsocks_total: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct Upstream {
    pub(crate) upstream_id: u64,
    pub(crate) route_kind: String,
    pub(crate) address: String,
    pub(crate) healthy: bool,
    pub(crate) fails: u64,
    pub(crate) last_check_age_secs: u64,
    #[serde(default)]
    pub(crate) effective_latency_ms: Option<f64>,
    #[serde(default)]
    pub(crate) dc: Vec<UpstreamDc>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct UpstreamDc {
    pub(crate) dc: u64,
    #[serde(default)]
    pub(crate) latency_ema_ms: Option<f64>,
    #[serde(default)]
    pub(crate) ip_preference: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RotateSecretResponse {
    pub(crate) user: UserInfo,
    pub(crate) secret: String,
}
