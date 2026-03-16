//! Чтение и обновление конфига telemt (/etc/telemt.toml).

use serde::Deserialize;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Mutex;
use toml_edit::{DocumentMut, Item, Table};

/// Параметры для генерации ссылки (host, port, tls_domain).
#[derive(Debug, Clone)]
pub struct TelemtLinkParams {
    pub host: String,
    pub port: u16,
    pub tls_domain: String,
}

/// Минимальная структура для чтения нужных полей telemt.
#[derive(Debug, Deserialize)]
struct TelemtConfigRaw {
    server: Option<ServerSection>,
    censorship: Option<CensorshipSection>,
}

#[derive(Debug, Deserialize)]
struct ServerSection {
    port: Option<u16>,
    listeners: Option<Vec<ListenerEntry>>,
}

#[derive(Debug, Deserialize)]
struct ListenerEntry {
    announce: Option<String>,
    announce_ip: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CensorshipSection {
    tls_domain: Option<String>,
}

/// Сервис для работы с конфигом telemt.
pub struct TelemtConfig {
    path: std::path::PathBuf,
    write_lock: Mutex<()>,
}

impl TelemtConfig {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            write_lock: Mutex::new(()),
        }
    }

    /// Читает параметры для генерации ссылки.
    pub fn read_link_params(&self) -> Result<TelemtLinkParams, anyhow::Error> {
        tracing::debug!("Reading link params from {}", self.path.display());
        let content = std::fs::read_to_string(&self.path)
            .map_err(|e| anyhow::anyhow!("Не удалось прочитать {}: {}", self.path.display(), e))?;

        let parsed: TelemtConfigRaw = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Ошибка парсинга telemt конфига: {}", e))?;

        let port = parsed.server.as_ref().and_then(|s| s.port).unwrap_or(443);

        let host = parsed
            .server
            .as_ref()
            .and_then(|s| s.listeners.as_ref())
            .and_then(|list| {
                list.iter()
                    .find_map(|l| l.announce.clone().or(l.announce_ip.clone()))
            })
            .ok_or_else(|| anyhow::anyhow!("Не найден announce/announce_ip в server.listeners"))?;

        let tls_domain = parsed
            .censorship
            .as_ref()
            .and_then(|c| c.tls_domain.clone())
            .ok_or_else(|| anyhow::anyhow!("Не задан censorship.tls_domain"))?;

        let params = TelemtLinkParams {
            host,
            port,
            tls_domain,
        };
        tracing::debug!(
            host = %params.host,
            port = params.port,
            "Link params loaded from telemt config"
        );
        Ok(params)
    }

    /// Добавляет или обновляет пользователя в [access.users].
    pub fn upsert_user(&self, username: &str, secret: &str) -> Result<(), anyhow::Error> {
        tracing::info!(username = username, "Upserting user in telemt config");
        let _lock = self
            .write_lock
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex poisoned: {}", e))?;

        let content = std::fs::read_to_string(&self.path)
            .map_err(|e| anyhow::anyhow!("Не удалось прочитать {}: {}", self.path.display(), e))?;

        let mut doc: DocumentMut = content
            .parse()
            .map_err(|e| anyhow::anyhow!("Ошибка парсинга TOML: {}", e))?;

        if doc.get("access").is_none() {
            doc["access"] = Item::Table(Table::new());
        }

        let access = doc
            .get_mut("access")
            .and_then(|a| a.as_table_mut())
            .ok_or_else(|| anyhow::anyhow!("Секция [access] имеет неверный тип"))?;

        if access.get("users").is_none() {
            access["users"] = Item::Table(Table::new());
        }

        let users = access
            .get_mut("users")
            .and_then(|u| u.as_table_mut())
            .ok_or_else(|| anyhow::anyhow!("Секция [access.users] имеет неверный тип"))?;

        users[username] = Item::Value(toml_edit::Value::from(secret));

        let new_content = doc.to_string();
        self.write_atomic(&new_content)?;
        tracing::info!(username = username, "User upserted in telemt config");
        Ok(())
    }

    /// Удаляет пользователя из [access.users].
    pub fn remove_user(&self, username: &str) -> Result<bool, anyhow::Error> {
        tracing::info!(username = username, "Removing user from telemt config");
        let _lock = self
            .write_lock
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex poisoned: {}", e))?;

        let content = std::fs::read_to_string(&self.path)
            .map_err(|e| anyhow::anyhow!("Не удалось прочитать {}: {}", self.path.display(), e))?;

        let mut doc: DocumentMut = content
            .parse()
            .map_err(|e| anyhow::anyhow!("Ошибка парсинга TOML: {}", e))?;

        let Some(access) = doc.get_mut("access").and_then(|a| a.as_table_mut()) else {
            tracing::warn!(
                username = username,
                "Section [access] is missing in telemt config while removing user"
            );
            return Ok(false);
        };

        let Some(users) = access.get_mut("users").and_then(|u| u.as_table_mut()) else {
            tracing::warn!(
                username = username,
                "Section [access.users] is missing in telemt config while removing user"
            );
            return Ok(false);
        };

        let existed = users.contains_key(username);
        users.remove(username);

        if existed {
            let new_content = doc.to_string();
            self.write_atomic(&new_content)?;
            tracing::info!(username = username, "User removed from telemt config");
        } else {
            tracing::warn!(username = username, "User was not found in telemt config");
        }
        Ok(existed)
    }

    fn write_atomic(&self, content: &str) -> Result<(), anyhow::Error> {
        // Дополнительная валидация финального текста перед заменой файла.
        let _: toml::Value = toml::from_str(content)
            .map_err(|e| anyhow::anyhow!("Невалидный TOML перед записью: {}", e))?;

        let parent = self.path.parent().unwrap_or(std::path::Path::new("."));
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        let tmp = parent.join(format!(".telemt.toml.{}.{}", std::process::id(), nonce));
        if let Err(err) = std::fs::write(&tmp, content) {
            if err.kind() == ErrorKind::PermissionDenied {
                // В некоторых окружениях есть права на изменение файла, но нет прав
                // на создание новых файлов в директории (например, /etc).
                tracing::warn!(
                    target_path = %self.path.display(),
                    "No permission to create temporary file; falling back to direct write"
                );
                std::fs::write(&self.path, content).map_err(|e| {
                    anyhow::anyhow!(
                        "Не удалось записать {} после fallback: {}",
                        self.path.display(),
                        e
                    )
                })?;
                return Ok(());
            }
            return Err(anyhow::anyhow!(
                "Не удалось записать временный файл: {}",
                err
            ));
        }
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| anyhow::anyhow!("Не удалось переименовать временный файл: {}", e))?;
        tracing::debug!(
            tmp_path = %tmp.display(),
            target_path = %self.path.display(),
            "telemt config written atomically"
        );
        Ok(())
    }
}
