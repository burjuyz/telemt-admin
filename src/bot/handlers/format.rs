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
        "{} · {} · до {}",
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
    let expires_label = format!("до {}", expires_at);

    format!(
        "🎟 Invite-токен\n\n\
         🔑 {}\n\
         ⚙️ {}\n\
         ⏳ {}\n\
         📊 {}\n\
         👤 {}\n\
         📅 создан {}",
        token.token,
        mode,
        expires_label,
        usage,
        created_by,
        format_date(token.created_at),
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

fn format_optional_usize(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn format_ip_list(values: &[String]) -> String {
    if values.is_empty() {
        "—".to_string()
    } else {
        values.join(", ")
    }
}

pub fn render_user_card_text(
    user: &RegistrationRequest,
    runtime_info: Option<&TelemtUserInfo>,
) -> String {
    let username = user
        .tg_username
        .as_deref()
        .map(|u| format!("@{}", u))
        .unwrap_or_else(|| "—".to_string());
    let telemt = user.telemt_username.as_deref().unwrap_or("—");
    let backend_mode = user.backend_mode.as_deref().unwrap_or("—");
    let last_sync = user
        .last_synced_at
        .map(format_timestamp)
        .unwrap_or_else(|| "—".to_string());
    let last_revision = user
        .last_seen_revision
        .as_deref()
        .map(|value| {
            if value.chars().count() > 24 {
                format!("{}...", value.chars().take(21).collect::<String>())
            } else {
                value.to_string()
            }
        })
        .unwrap_or_else(|| "—".to_string());
    let sync_error = user.last_sync_error.as_deref().unwrap_or("нет");
    let runtime_source = runtime_info
        .map(|info| info.source.as_str())
        .unwrap_or("нет данных");
    let current_connections = runtime_info
        .and_then(|info| info.current_connections)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string());
    let active_unique_ips = runtime_info
        .and_then(|info| info.active_unique_ips)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string());
    let recent_unique_ips = runtime_info
        .and_then(|info| info.recent_unique_ips)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_string());
    let active_ip_list = runtime_info
        .map(|info| format_ip_list(&info.active_unique_ips_list))
        .unwrap_or_else(|| "—".to_string());
    let recent_ip_list = runtime_info
        .map(|info| format_ip_list(&info.recent_unique_ips_list))
        .unwrap_or_else(|| "—".to_string());
    let total_octets = runtime_info
        .map(|info| format_optional_bytes(info.total_octets))
        .unwrap_or_else(|| "—".to_string());
    let user_ad_tag = runtime_info
        .and_then(|info| info.user_ad_tag.as_deref())
        .unwrap_or("—");
    let max_tcp_conns = runtime_info
        .map(|info| format_optional_usize(info.max_tcp_conns))
        .unwrap_or_else(|| "—".to_string());
    let data_quota = runtime_info
        .map(|info| format_optional_bytes(info.data_quota_bytes))
        .unwrap_or_else(|| "—".to_string());
    let max_unique_ips = runtime_info
        .map(|info| format_optional_usize(info.max_unique_ips))
        .unwrap_or_else(|| "—".to_string());
    let expiration = runtime_info
        .and_then(|info| info.expiration_rfc3339.as_deref())
        .unwrap_or("—");
    let links_count = runtime_info
        .map(|info| info.links.len().to_string())
        .unwrap_or_else(|| "0".to_string());

    format!(
        "👤 {}\n\n\
         🆔 {}\n\
         📱 {}\n\
         📋 статус: {}\n\
         🔗 telemt: {}\n\
         🧩 backend: {}\n\
         🔁 sync: {}\n\
         🪪 revision: {}\n\
         ⚠️ sync error: {}\n\
         \n\
         📡 runtime source: {}\n\
         🔌 live connections: {}\n\
         🌐 active unique IPs: {}\n\
         🕘 recent unique IPs: {}\n\
         📋 active IP list: {}\n\
         📋 recent IP list: {}\n\
         📦 traffic: {}\n\
         🏷 ad tag: {}\n\
         🛡 max TCP: {}\n\
         🧮 quota: {}\n\
         🌍 max unique IPs: {}\n\
         ⏳ expires: {}\n\
         🔗 links: {}\n\
         📅 {}",
        user_display_name(user),
        user.tg_user_id,
        username,
        user.status,
        telemt,
        backend_mode,
        last_sync,
        last_revision,
        sync_error,
        runtime_source,
        current_connections,
        active_unique_ips,
        recent_unique_ips,
        active_ip_list,
        recent_ip_list,
        total_octets,
        user_ad_tag,
        max_tcp_conns,
        data_quota,
        max_unique_ips,
        expiration,
        links_count,
        format_timestamp(user.created_at),
    )
}

pub fn usage_guide_text() -> &'static str {
    r#"Как подключиться к прокси:

1) Получите ссылку через команду /link.
2) Нажмите на ссылку — Telegram автоматически предложит добавить прокси.
3) Подтвердите добавление.

Если доступа ещё нет, начните с /start и введите invite-токен.
Если не получается, обратитесь к администратору."#
}
