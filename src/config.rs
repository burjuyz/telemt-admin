//! Конфигурация telemt-admin бота.

use serde::Deserialize;
use std::path::PathBuf;

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

impl Config {
    pub fn load(path: &std::path::Path) -> Result<Self, anyhow::Error> {
        tracing::debug!("Loading config from {}", path.display());
        let content = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!("Не удалось прочитать конфиг {}: {}", path.display(), e)
        })?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Ошибка парсинга конфига: {}", e))?;
        if let Some(ttl) = config.security.wizard_state_ttl_seconds
            && ttl <= 0
        {
            return Err(anyhow::anyhow!(
                "security.wizard_state_ttl_seconds должен быть положительным числом секунд"
            ));
        }
        tracing::info!(
            admin_count = config.admin_ids.len(),
            telemt_config_path = %config.telemt_config_path.display(),
            db_path = %config.db_path.display(),
            service_name = %config.service_name,
            users_page_size = config.users_page_size,
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
                anyhow::anyhow!("Не задан bot_token в конфиге и TELOXIDE_TOKEN в окружении")
            })
    }

    pub fn is_admin(&self, user_id: i64) -> bool {
        self.admin_ids.contains(&user_id)
    }
}
