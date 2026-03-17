use super::state::{BotState, sender_user_id};
use anyhow::anyhow;
use image::{DynamicImage, ImageFormat, Luma};
use qrcode::QrCode;
use std::io::Cursor;
use teloxide::prelude::*;
use teloxide::types::{MessageId, ParseMode};

pub type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

pub enum CreateTarget {
    UserId(i64),
    Username(String),
}

pub fn parse_create_target(arg: &str) -> Option<CreateTarget> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(user_id) = trimmed.parse::<i64>() {
        return Some(CreateTarget::UserId(user_id));
    }

    let username = trimmed.strip_prefix('@')?.trim();
    if username.is_empty() {
        return None;
    }

    Some(CreateTarget::Username(username.to_string()))
}

pub fn parse_start_token(text: &str) -> Option<String> {
    let mut parts = text.split_whitespace();
    let command = parts.next()?;
    if !command.starts_with("/start") {
        return None;
    }
    let token = parts.next()?.trim();
    if token.is_empty() {
        return None;
    }

    let decoded = match urlencoding::decode(token) {
        Ok(value) => value.into_owned(),
        Err(_) => token.to_string(),
    };
    let normalized = decoded.trim().trim_matches('`').trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub fn callback_message_target(q: &CallbackQuery) -> Option<(ChatId, MessageId)> {
    q.message.as_ref().map(|msg| (msg.chat().id, msg.id()))
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn format_admin_backend_error(
    action: &str,
    error: &(dyn std::error::Error + Send + Sync),
) -> String {
    format!(
        "<b>Ошибка backend</b>\n\n<b>Операция:</b> {}\n<b>Причина:</b>\n<pre>{}</pre>",
        escape_html(action),
        escape_html(&error.to_string())
    )
}

pub async fn send_admin_backend_error(
    bot: &Bot,
    chat_id: ChatId,
    action: &str,
    error: &(dyn std::error::Error + Send + Sync),
) {
    let text = format_admin_backend_error(action, error);
    if let Err(send_error) = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .await
    {
        tracing::warn!(
            chat_id = chat_id.0,
            action = action,
            error = %send_error,
            "Не удалось отправить администратору сообщение об ошибке backend"
        );
    }
}

pub fn build_bot_start_link(bot_username: &str, token: &str) -> String {
    let normalized = bot_username.trim_start_matches('@');
    format!("https://t.me/{}?start={}", normalized, token)
}

pub fn build_user_qr_png_bytes(payload: &str) -> Result<Vec<u8>, anyhow::Error> {
    let qr = QrCode::new(payload.as_bytes())?;
    let image = qr
        .render::<Luma<u8>>()
        .quiet_zone(true)
        .min_dimensions(512, 512)
        .build();
    let mut bytes = Vec::new();
    {
        let mut cursor = Cursor::new(&mut bytes);
        DynamicImage::ImageLuma8(image).write_to(&mut cursor, ImageFormat::Png)?;
    }
    Ok(bytes)
}

pub async fn require_admin_callback(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
) -> Result<Option<i64>, anyhow::Error> {
    let admin_id = q.from.id.0 as i64;
    if !state.config.is_admin(admin_id) {
        bot.answer_callback_query(q.id.clone())
            .text("Недостаточно прав")
            .show_alert(true)
            .await?;
        return Ok(None);
    }
    Ok(Some(admin_id))
}

pub fn user_id_or_reply(msg: &Message) -> Result<i64, anyhow::Error> {
    sender_user_id(msg).ok_or_else(|| anyhow!("Не удалось определить пользователя отправителя"))
}
