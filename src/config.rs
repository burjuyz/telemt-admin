//! Конфигурация telemt-admin бота.

use serde::Deserialize;
use std::path::PathBuf;

use crate::runtime::RuntimeMode;

/// Путь к конфигу по умолчанию.
pub const DEFAULT_CONFIG_PATH: &str = "/etc/telemt-admin.toml";

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Токен Telegram бота (или через TELOXIDE_TOKEN)
    pub bot_token: Option<String>,
    /// Список Telegram user_id администраторов
    pub admin_ids: Vec<i64>,
    /// Путь к конфигу telemt (по умолчанию /etc/telemt.toml)
    #[serde(default = "default_telemt_config_path")]
    pub telemt_config_path: PathBuf,
    /// Путь к SQLite БД (по умолчанию /var/lib/telemt-admin/state.db)
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    /// Имя systemd-сервиса telemt
    #[serde(default = "default_service_name")]
    pub service_name: String,
    /// Размер страницы в списке активных пользователей
    #[serde(default = "default_users_page_size")]
    pub users_page_size: i64,
    /// Политики безопасности invite-токенов
    #[serde(default)]
    pub security: SecurityConfig,
    /// Настройки control API telemt
    #[serde(default)]
    pub telemt_api: TelemtApiConfig,
    /// Настройки уведомлений и фонового мониторинга
    #[serde(default)]
    pub notifications: NotificationsConfig,
    /// Режим управления процессом telemt на хосте (systemd / внешний supervisor / без unit)
    #[serde(default)]
    pub runtime: Option<RuntimeSection>,
}

/// Секция `[runtime]` в `telemt-admin.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeSection {
    #[serde(default = "default_runtime_mode")]
    pub mode: RuntimeMode,
    /// Имя systemd unit для `mode = "systemd"` (если не задано — используется верхнеуровневый `service_name`)
    #[serde(default)]
    pub service_name: Option<String>,
    /// Подпись для UI при `mode = "external"`
    #[serde(default)]
    pub label: Option<String>,
}

pub(crate) fn default_runtime_mode() -> RuntimeMode {
    RuntimeMode::Systemd
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_token_days")]
    pub default_token_days: i64,
    #[serde(default = "default_max_token_days")]
    pub max_token_days: i64,
    #[serde(default = "default_allow_auto_approve_tokens")]
    pub allow_auto_approve_tokens: bool,
    pub wizard_state_ttl_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelemtApiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_telemt_api_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub auth_header: Option<String>,
    #[serde(default = "default_telemt_api_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_allow_file_fallback")]
    pub allow_file_fallback: bool,
    #[serde(default = "default_prefer_api_links")]
    pub prefer_api_links: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationsConfig {
    #[serde(default = "default_notifications_enabled")]
    pub enabled: bool,
    #[serde(default = "default_health_check_interval_secs")]
    pub health_check_interval_secs: u64,
    #[serde(default = "default_notify_on_health_change")]
    pub notify_on_health_change: bool,
    #[serde(default = "default_notify_on_runtime_alerts")]
    pub notify_on_runtime_alerts: bool,
    #[serde(default = "default_notify_on_new_request")]
    pub notify_on_new_request: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            default_token_days: default_token_days(),
            max_token_days: default_max_token_days(),
            allow_auto_approve_tokens: default_allow_auto_approve_tokens(),
            wizard_state_ttl_seconds: None,
        }
    }
}

impl Default for TelemtApiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_telemt_api_base_url(),
            auth_header: None,
            timeout_ms: default_telemt_api_timeout_ms(),
            allow_file_fallback: default_allow_file_fallback(),
            prefer_api_links: default_prefer_api_links(),
        }
    }
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            enabled: default_notifications_enabled(),
            health_check_interval_secs: default_health_check_interval_secs(),
            notify_on_health_change: default_notify_on_health_change(),
            notify_on_runtime_alerts: default_notify_on_runtime_alerts(),
            notify_on_new_request: default_notify_on_new_request(),
        }
    }
}

fn default_telemt_config_path() -> PathBuf {
    PathBuf::from("/etc/telemt.toml")
}

fn default_db_path() -> PathBuf {
    PathBuf::from("/var/lib/telemt-admin/state.db")
}

fn default_service_name() -> String {
    "telemt.service".to_string()
}

fn default_users_page_size() -> i64 {
    10
}

fn default_token_days() -> i64 {
    14
}

fn default_max_token_days() -> i64 {
    180
}

fn default_allow_auto_approve_tokens() -> bool {
    true
}

fn default_telemt_api_base_url() -> String {
    "http://127.0.0.1:9091".to_string()
}

fn default_telemt_api_timeout_ms() -> u64 {
    5_000
}

fn default_allow_file_fallback() -> bool {
    true
}

fn default_prefer_api_links() -> bool {
    true
}

fn default_notifications_enabled() -> bool {
    true
}

fn default_health_check_interval_secs() -> u64 {
    60
}

fn default_notify_on_health_change() -> bool {
    true
}

fn default_notify_on_runtime_alerts() -> bool {
    true
}

fn default_notify_on_new_request() -> bool {
    true
}

impl Config {
    pub fn load(path: &std::path::Path) -> Result<Self, anyhow::Error> {
        tracing::debug!("Loading config from {}", path.display());
        let content = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!("Не удалось прочитать конфиг {}: {}", path.display(), e)
        })?;
        let mut config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Ошибка парсинга конфига: {}", e))?;
        let env_override_keys = crate::env_config_overlay::apply(&mut config)?;
        if let Some(ttl) = config.security.wizard_state_ttl_seconds
            && ttl <= 0
        {
            return Err(anyhow::anyhow!(
                "security.wizard_state_ttl_seconds должен быть положительным числом секунд"
            ));
        }
        if config.notifications.health_check_interval_secs == 0 {
            return Err(anyhow::anyhow!(
                "notifications.health_check_interval_secs должен быть положительным"
            ));
        }
        tracing::info!(
            admin_count = config.admin_ids.len(),
            telemt_config_path = %config.telemt_config_path.display(),
            db_path = %config.db_path.display(),
            service_name = %config.service_name,
            env_overrides_count = env_override_keys.len(),
            env_override_keys = ?env_override_keys,
            runtime_mode = %config.effective_runtime_mode().as_str(),
            runtime_systemd_unit = %config.effective_systemd_unit(),
            users_page_size = config.users_page_size,
            telemt_api_enabled = config.telemt_api.enabled,
            telemt_api_base_url = %config.telemt_api.base_url,
            telemt_api_timeout_ms = config.telemt_api.timeout_ms,
            telemt_api_allow_file_fallback = config.telemt_api.allow_file_fallback,
            telemt_api_prefer_api_links = config.telemt_api.prefer_api_links,
            notifications_enabled = config.notifications.enabled,
            notifications_health_check_interval_secs = config.notifications.health_check_interval_secs,
            notifications_notify_on_health_change = config.notifications.notify_on_health_change,
            notifications_notify_on_runtime_alerts = config.notifications.notify_on_runtime_alerts,
            notifications_notify_on_new_request = config.notifications.notify_on_new_request,
            security_default_days = config.security.default_token_days,
            security_max_days = config.security.max_token_days,
            allow_auto_approve_tokens = config.security.allow_auto_approve_tokens,
            wizard_state_ttl_seconds = ?config.security.wizard_state_ttl_seconds,
            "Config parsed successfully"
        );
        Ok(config)
    }

    pub fn bot_token(&self) -> Result<String, anyhow::Error> {
        self.bot_token
            .clone()
            .or_else(|| std::env::var("TELOXIDE_TOKEN").ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Не задан bot_token (в TOML, TELEMT_ADMIN__BOT_TOKEN или TELOXIDE_TOKEN)"
                )
            })
    }

    pub fn is_admin(&self, user_id: i64) -> bool {
        self.admin_ids.contains(&user_id)
    }

    /// Режим runtime: при отсутствии `[runtime]` считается `systemd` (как раньше).
    pub fn effective_runtime_mode(&self) -> RuntimeMode {
        self.runtime
            .as_ref()
            .map(|r| r.mode)
            .unwrap_or(RuntimeMode::Systemd)
    }

    /// Имя unit для `systemctl` при режиме systemd.
    pub fn effective_systemd_unit(&self) -> String {
        self.runtime
            .as_ref()
            .and_then(|r| r.service_name.as_ref())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.service_name.clone())
    }

    /// Метка для UI при `external`.
    pub fn effective_external_label(&self) -> Option<String> {
        self.runtime.as_ref().and_then(|r| {
            r.label
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
    }
}
