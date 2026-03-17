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

    format!(
        "👤 {}\n\n\
         🆔 {}\n\
         📱 {}\n\
         📋 статус: {}\n\
         🔗 telemt: {}\n\
         📅 {}",
        user_display_name(user),
        user.tg_user_id,
        username,
        user.status,
        telemt,
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
