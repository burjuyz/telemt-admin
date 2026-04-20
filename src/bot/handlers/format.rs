use crate::db::{InviteToken, RegistrationRequest};
use crate::telemt_backend::TelemtUserInfo;
use chrono::{DateTime, Local, Utc};

pub fn format_date(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| dt.with_timezone(&Local).format("%d.%m.%Y").to_string())
        .unwrap_or_else(|| "—".to_string())
}

pub fn format_mode(auto_approve: bool) -> &'static str {
    if auto_approve {
        "АВТОПОДТВЕРЖДЕНИЕ 🚀"
    } else {
        "Ручной ✅"
    }
}

pub fn format_timestamp(ts: i64) -> String {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S %:z")
                .to_string()
        })
        .unwrap_or_else(|| format!("Некорректный timestamp: {}", ts))
}

pub fn user_display_name(user: &RegistrationRequest) -> String {
    user.tg_display_name
        .clone()
        .or_else(|| {
            user.tg_username
                .as_ref()
                .map(|username| format!("@{}", username))
        })
        .or_else(|| user.telemt_username.clone())
        .unwrap_or_else(|| format!("tg_{}", user.tg_user_id))
}

pub fn render_invite_token_button_title(token: &InviteToken) -> String {
    let mode = if token.auto_approve { "AUTO" } else { "MANUAL" };
    format!(
        "{} · {} · invite до {}",
        token.token,
        mode,
        format_date(token.expires_at)
    )
}

pub fn render_invite_token_card_text(token: &InviteToken) -> String {
    let mode = if token.auto_approve { "AUTO" } else { "MANUAL" };
    let usage = token
        .max_usage
        .map(|max| format!("{}/{}", token.usage_count, max))
        .unwrap_or_else(|| format!("{}/∞", token.usage_count));
    let created_by = token
        .created_by
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string());
    let expires_at = format_date(token.expires_at);

    let limits_text = {
        let mut parts = Vec::new();
        if let Some(days) = token.default_expiration_days {
            parts.push(format!("доступ {} дн.", days));
        }
        if let Some(ips) = token.default_max_unique_ips {
            parts.push(format!("IP: {}", ips));
        }
        if let Some(quota) = token.default_data_quota_bytes {
            let gb = quota as f64 / 1_073_741_824.0;
            parts.push(format!("{:.1} GB", gb));
        }
        if parts.is_empty() {
            "по умолчанию".to_string()
        } else {
            parts.join(", ")
        }
    };

    format!(
        "🎟 Invite-токен\n\n\
         🔑 {}\n\
         ⚙️ {}\n\
         ⏳ Срок действия ссылки (invite): до {}\n\
         📊 Активаций по ссылке: {}\n\
         👤 {}\n\
         📅 создан {}\n\n\
         📈 Лимиты пользователя: {}",
        token.token,
        mode,
        expires_at,
        usage,
        created_by,
        format_date(token.created_at),
        limits_text,
    )
}

pub fn format_bytes_human(value: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let value_f = value as f64;
    if value_f >= TB {
        format!("{:.2} TB", value_f / TB)
    } else if value_f >= GB {
        format!("{:.2} GB", value_f / GB)
    } else if value_f >= MB {
        format!("{:.2} MB", value_f / MB)
    } else if value_f >= KB {
        format!("{:.2} KB", value_f / KB)
    } else {
        format!("{value} B")
    }
}

fn format_optional_bytes(value: Option<u64>) -> String {
    value
        .map(format_bytes_human)
        .unwrap_or_else(|| "—".to_string())
}

pub fn render_user_card_text(
    user: &RegistrationRequest,
    runtime_info: Option<&TelemtUserInfo>,
) -> String {
    let display_name = user_display_name(user);
    let status = user.status.to_string();
    let tg_user_id = user.tg_user_id;

    let current_connections = runtime_info
        .and_then(|info| info.current_connections)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "0".to_string());
    let active_unique_ips = runtime_info
        .and_then(|info| info.active_unique_ips)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "0".to_string());
    let recent_unique_ips = runtime_info
        .and_then(|info| info.recent_unique_ips)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "0".to_string());
    let total_octets = runtime_info
        .map(|info| format_optional_bytes(info.total_octets))
        .unwrap_or_else(|| "0 B".to_string());

    let max_tcp = runtime_info
        .and_then(|info| info.max_tcp_conns)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "∞".to_string());
    let max_ips = runtime_info
        .and_then(|info| info.max_unique_ips)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "∞".to_string());
    let quota = runtime_info
        .map(|info| format_optional_bytes(info.data_quota_bytes))
        .unwrap_or_else(|| "∞".to_string());

    let expiration = runtime_info
        .and_then(|info| info.expiration_rfc3339.as_deref())
        .unwrap_or("Не задано");

    let username = user.telemt_username.as_deref().unwrap_or("—");

    let last_sync = user
        .last_synced_at
        .map(format_timestamp)
        .unwrap_or_else(|| "—".to_string());

    let mut lines = vec![
        format!("👤 {}\n", display_name),
        format!("🆔 {} | 📋 {}\n", tg_user_id, status),
        format!(
            "📡 ● {} соединений | 🌐 {} active IP | 🕘 {} recent IP\n",
            current_connections, active_unique_ips, recent_unique_ips
        ),
        format!("📦 {} трафика\n", total_octets),
        String::new(),
        format!(
            "🛡 TCP: {} | 🌍 IP: {} | 🧮 Квота: {}\n",
            max_tcp, max_ips, quota
        ),
    ];

    if let Some(info) = runtime_info {
        let mut warnings = Vec::new();
        if let (Some(cur), Some(limit)) = (info.current_connections, info.max_tcp_conns)
            && cur > limit as u64
        {
            warnings.push(format!("TCP: {}/{} (превышен!)", cur, limit));
        }
        if let (Some(cur), Some(limit)) = (info.active_unique_ips, info.max_unique_ips)
            && cur > limit
        {
            warnings.push(format!("IP: {}/{} (превышен!)", cur, limit));
        }
        if !warnings.is_empty() {
            lines.push(format!("⚠️ {}\n", warnings.join(", ")));
        }
    }

    lines.push(format!("⏳ {}\n", expiration));
    lines.push(format!("🔗 {}\n", username));
    lines.push(format!("⏱ synced {}", last_sync));

    lines.join("")
}

pub fn usage_guide_text() -> &'static str {
    r#"Как подключиться к прокси:

1) Откройте главный экран (/start) и нажмите «Моя ссылка».
2) Telegram автоматически предложит добавить прокси.
3) Подтвердите добавление.

Если доступа ещё нет, начните с /start и введите invite-токен.
Если не получается, обратитесь к администратору."#
}

pub fn render_upstreams_summary_text(
    summary: Option<&crate::telemt_backend::TelemtUpstreamsSummary>,
    error: Option<&str>,
) -> String {
    match summary {
        Some(summary) => {
            let mut lines = vec![
                "📡 Upstreams".to_string(),
                String::new(),
                format!(
                    "✅ {}/{} healthy | {}",
                    summary.healthy_total, summary.configured_total, summary.route_kinds
                ),
                String::new(),
            ];

            for upstream in &summary.upstreams {
                let dc_parts: Vec<String> = upstream
                    .dc_list
                    .iter()
                    .map(|dc| {
                        let status = if dc.latency_ms.is_some() { "✅" } else { "❌" };
                        let latency = dc
                            .latency_ms
                            .map(|ms| format!("{:.0}ms", ms))
                            .unwrap_or_else(|| "—".to_string());
                        format!("DC{}: {} {}", dc.dc, status, latency)
                    })
                    .collect();
                lines.push(format!("🌐 {}", dc_parts.join(" | ")));
            }

            if let Some(first_upstream) = summary.upstreams.first() {
                lines.push(String::new());
                lines.push(format!(
                    "⏱ Последняя проверка: {} сек назад",
                    first_upstream.last_check_age_secs
                ));
            }

            lines.join("\n")
        }
        None => {
            let mut text = "📡 Upstreams\n\nФункция отключена или недоступна в telemt.".to_string();
            if let Some(err) = error {
                text.push_str("\n\nПричина: ");
                text.push_str(err);
            }
            text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{RegistrationRequest, RequestStatus};
    use crate::telemt_backend::{TelemtBackendMode, TelemtUserInfo};

    fn sample_request() -> RegistrationRequest {
        RegistrationRequest {
            id: 1,
            tg_user_id: 42,
            tg_username: Some("alice".to_string()),
            tg_display_name: Some("Alice".to_string()),
            status: RequestStatus::Approved,
            telemt_username: Some("tg_42".to_string()),
            secret: Some("secret".to_string()),
            created_at: 1_700_000_000,
            backend_mode: Some("control_api".to_string()),
            last_sync_error: None,
            last_seen_revision: Some("rev-123".to_string()),
            last_synced_at: Some(1_700_000_100),
            invite_token_id: Some(7),
        }
    }

    fn sample_runtime_info() -> TelemtUserInfo {
        TelemtUserInfo {
            source: TelemtBackendMode::ControlApi,
            user_ad_tag: Some("promo".to_string()),
            max_tcp_conns: Some(10),
            expiration_rfc3339: Some("2026-04-01T00:00:00Z".to_string()),
            data_quota_bytes: Some(4096),
            max_unique_ips: Some(3),
            current_connections: Some(2),
            active_unique_ips: Some(1),
            active_unique_ips_list: vec!["1.1.1.1".to_string()],
            recent_unique_ips: Some(2),
            recent_unique_ips_list: vec!["1.1.1.1".to_string(), "2.2.2.2".to_string()],
            total_octets: Some(8192),
            links: vec!["link-1".to_string(), "link-2".to_string()],
        }
    }

    #[test]
    fn format_bytes_human_formats_thresholds() {
        assert_eq!(format_bytes_human(512), "512 B");
        assert_eq!(format_bytes_human(2048), "2.00 KB");
        assert_eq!(format_bytes_human(5 * 1024 * 1024), "5.00 MB");
    }

    #[test]
    fn render_user_card_text_includes_runtime_and_sync_data() {
        let text = render_user_card_text(&sample_request(), Some(&sample_runtime_info()));

        assert!(text.contains("Alice"));
        assert!(text.contains("42"));
        assert!(text.contains("🔗 tg_"));
        assert!(text.contains("📡 ●"));
        assert!(text.contains("соединений"));
    }

    #[test]
    fn render_user_card_text_shows_limits() {
        let req = sample_request();
        let mut info = sample_runtime_info();
        info.max_tcp_conns = Some(10);
        info.current_connections = Some(5);
        let text = render_user_card_text(&req, Some(&info));

        assert!(text.contains("TCP: 10"));
        assert!(text.contains("соединений"));
    }

    #[test]
    fn usage_guide_text_mentions_start_and_link() {
        let text = usage_guide_text();

        assert!(text.contains("/start"));
        assert!(text.contains("Моя ссылка"));
    }
}
