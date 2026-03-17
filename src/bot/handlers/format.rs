use crate::db::{InviteToken, RegistrationRequest};
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

pub fn render_user_card_text(user: &RegistrationRequest) -> String {
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
