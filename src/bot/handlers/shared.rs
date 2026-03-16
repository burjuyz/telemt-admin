use super::format::{format_timestamp, user_display_name};
use super::state::{sender_user_id, telemt_username, BotState};
use crate::db::{
    ConsumedInviteToken, RegisterResult, RegistrationRequest, TokenConsumeError, TokenMode,
};
use crate::link::{build_proxy_link, generate_user_secret};
use anyhow::anyhow;
use image::{DynamicImage, ImageFormat, Luma};
use qrcode::QrCode;
use std::io::Cursor;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, InputFile};

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

pub fn parse_callback_request_id(data: &str, prefix: &str) -> Result<i64, anyhow::Error> {
    data.strip_prefix(prefix)
        .ok_or_else(|| anyhow!("Некорректный callback payload"))?
        .parse::<i64>()
        .map_err(|_| anyhow!("Некорректный request_id"))
}

pub fn parse_callback_user_action(data: &str, prefix: &str) -> Result<(i64, i64), anyhow::Error> {
    let payload = data
        .strip_prefix(prefix)
        .ok_or_else(|| anyhow!("Некорректный callback payload"))?;
    let mut parts = payload.split(':');
    let tg_user_id = parts
        .next()
        .ok_or_else(|| anyhow!("Не указан tg_user_id"))?
        .parse::<i64>()
        .map_err(|_| anyhow!("Некорректный tg_user_id"))?;
    let page = parts
        .next()
        .ok_or_else(|| anyhow!("Не указан номер страницы"))?
        .parse::<i64>()
        .map_err(|_| anyhow!("Некорректный номер страницы"))?;
    Ok((tg_user_id, page.max(1)))
}

pub fn parse_callback_page(data: &str, prefix: &str) -> Result<i64, anyhow::Error> {
    data.strip_prefix(prefix)
        .ok_or_else(|| anyhow!("Некорректный callback payload"))?
        .parse::<i64>()
        .map(|page| page.max(1))
        .map_err(|_| anyhow!("Некорректный номер страницы"))
}

pub fn callback_message_target(q: &CallbackQuery) -> Option<(ChatId, teloxide::types::MessageId)> {
    q.message.as_ref().map(|msg| (msg.chat().id, msg.id()))
}

pub fn build_bot_start_link(bot_username: &str, token: &str) -> String {
    let normalized = bot_username.trim_start_matches('@');
    format!("https://t.me/{}?start={}", normalized, token)
}

fn reload_telemt_after_config_change(state: &BotState) {
    let reload = state.service.notify_config_reloaded();
    if !reload.success {
        tracing::warn!(stderr = %reload.stderr, "telemt config reload/restart had issues");
    }
}

pub async fn mark_user_waiting_for_invite(state: &BotState, tg_user_id: i64) {
    state.awaiting_invite_users.lock().await.insert(tg_user_id);
}

pub async fn unmark_user_waiting_for_invite(state: &BotState, tg_user_id: i64) {
    state.awaiting_invite_users.lock().await.remove(&tg_user_id);
}

pub async fn is_user_waiting_for_invite(state: &BotState, tg_user_id: i64) -> bool {
    state
        .awaiting_invite_users
        .lock()
        .await
        .contains(&tg_user_id)
}

pub async fn notify_auto_approve(
    bot: &Bot,
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
    token: &ConsumedInviteToken,
) {
    let mode_label = match token.mode {
        TokenMode::AutoApprove => "auto",
        TokenMode::Manual => "manual",
    };
    let text = format!(
        "✅ Автоподключение по токену\n\
         User ID: {}\n\
         Username: @{}\n\
         Имя: {}\n\
         Token: {}\n\
         Token ID: {}\n\
         Mode: {}\n\
         Expires: {}\n\
         Usage: {}/{}\n\
         Created by: {}",
        tg_user_id,
        tg_username.unwrap_or("—"),
        tg_display_name.unwrap_or("—"),
        token.token,
        token.id,
        mode_label,
        format_timestamp(token.expires_at),
        token.usage_count,
        token
            .max_usage
            .map(|value| value.to_string())
            .unwrap_or_else(|| "∞".to_string()),
        token
            .created_by
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".to_string())
    );

    for admin_id in &state.config.admin_ids {
        if let Err(error) = bot.send_message(ChatId(*admin_id), text.clone()).await {
            tracing::warn!(
                admin_id = *admin_id,
                error = %error,
                "Не удалось отправить аудит автоподключения"
            );
        }
    }
}

pub async fn notify_admins(
    bot: &Bot,
    state: &BotState,
    req: &RegistrationRequest,
) -> HandlerResult {
    let text = format!(
        "📋 Новая заявка #{}:\n\
         User ID: {}\n\
         Username: @{}\n\
         Имя: {}\n\
         Время: {}",
        req.id,
        req.tg_user_id,
        req.tg_username.as_deref().unwrap_or("—"),
        req.tg_display_name.as_deref().unwrap_or("—"),
        format_timestamp(req.created_at),
    );

    let kb = crate::bot::keyboards::approve_reject_buttons(req.id);

    for admin_id in &state.config.admin_ids {
        if let Err(e) = bot
            .send_message(ChatId(*admin_id), text.clone())
            .reply_markup(kb.clone())
            .await
        {
            tracing::warn!(
                "Не удалось отправить уведомление админу {}: {}",
                admin_id,
                e
            );
        }
    }
    Ok(())
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

pub async fn approve_request_and_build_link(
    state: &BotState,
    request_id: i64,
) -> Result<Option<(RegistrationRequest, String)>, anyhow::Error> {
    let request = match state.db.get_pending_by_id(request_id).await? {
        Some(request) => request,
        None => return Ok(None),
    };

    let telemt_user = telemt_username(request.tg_user_id);
    let user_secret = generate_user_secret();

    state.telemt_cfg.upsert_user(&telemt_user, &user_secret)?;
    reload_telemt_after_config_change(state);
    if state
        .db
        .approve(request_id, &telemt_user, &user_secret)
        .await?
        .is_none()
    {
        return Ok(None);
    }

    let link_params = state.telemt_cfg.read_link_params()?;
    let proxy_link = build_proxy_link(&link_params, &user_secret)?;
    Ok(Some((request, proxy_link)))
}

pub async fn approve_user_direct_and_build_link(
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
) -> Result<String, anyhow::Error> {
    let telemt_user = telemt_username(tg_user_id);
    let secret = generate_user_secret();
    state.telemt_cfg.upsert_user(&telemt_user, &secret)?;
    reload_telemt_after_config_change(state);
    state
        .db
        .set_approved(
            tg_user_id,
            tg_username,
            tg_display_name,
            &telemt_user,
            &secret,
        )
        .await?;

    let params = state.telemt_cfg.read_link_params()?;
    build_proxy_link(&params, &secret).map_err(anyhow::Error::from)
}

pub async fn process_invite_token(
    bot: &Bot,
    msg: &Message,
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
    token: &str,
) -> HandlerResult {
    let consumed = match state.db.consume_invite_token(token).await {
        Ok(token_payload) => token_payload,
        Err(TokenConsumeError::NotFound) => {
            bot.send_message(
                msg.chat.id,
                "Токен не найден. Проверьте код и попробуйте снова.",
            )
            .await?;
            return Ok(());
        }
        Err(TokenConsumeError::Revoked) => {
            bot.send_message(msg.chat.id, "Этот токен отозван администратором.")
                .await?;
            return Ok(());
        }
        Err(TokenConsumeError::Expired) => {
            bot.send_message(msg.chat.id, "Срок действия токена истёк.")
                .await?;
            return Ok(());
        }
        Err(TokenConsumeError::UsageLimitReached) => {
            bot.send_message(msg.chat.id, "Лимит использований токена исчерпан.")
                .await?;
            return Ok(());
        }
    };

    tracing::info!(
        tg_user_id = tg_user_id,
        token = %consumed.token,
        token_id = consumed.id,
        mode = ?consumed.mode,
        usage_count = consumed.usage_count,
        max_usage = ?consumed.max_usage,
        expires_at = consumed.expires_at,
        "Токен успешно применён"
    );

    match consumed.mode {
        TokenMode::Manual => {
            let result = state
                .db
                .register_or_get(tg_user_id, tg_username, tg_display_name)
                .await?;
            match result {
                RegisterResult::Approved(secret) => {
                    let params = state.telemt_cfg.read_link_params()?;
                    let link = build_proxy_link(&params, &secret)?;
                    bot.send_message(msg.chat.id, format!("Ваша ссылка на прокси:\n\n{}", link))
                        .reply_markup(crate::bot::keyboards::user_menu())
                        .await?;
                    unmark_user_waiting_for_invite(state, tg_user_id).await;
                }
                RegisterResult::Rejected => {
                    bot.send_message(
                        msg.chat.id,
                        "Ваша заявка на регистрацию отклонена администратором.",
                    )
                    .reply_markup(crate::bot::keyboards::user_menu())
                    .await?;
                    unmark_user_waiting_for_invite(state, tg_user_id).await;
                }
                RegisterResult::AlreadyPending => {
                    bot.send_message(
                        msg.chat.id,
                        "Ваша заявка уже на рассмотрении. Ожидайте подтверждения администратора.",
                    )
                    .reply_markup(crate::bot::keyboards::user_menu())
                    .await?;
                    unmark_user_waiting_for_invite(state, tg_user_id).await;
                }
                RegisterResult::NewPending(ref req) => {
                    bot.send_message(msg.chat.id, "Заявка отправлена. Ожидайте подтверждения.")
                        .reply_markup(crate::bot::keyboards::user_menu())
                        .await?;
                    notify_admins(bot, state, req).await?;
                    unmark_user_waiting_for_invite(state, tg_user_id).await;
                }
            }
        }
        TokenMode::AutoApprove => {
            let link =
                approve_user_direct_and_build_link(state, tg_user_id, tg_username, tg_display_name)
                    .await?;
            bot.send_message(
                msg.chat.id,
                format!("Доступ одобрен! Ваша ссылка для подключения:\n\n{}", link),
            )
            .reply_markup(crate::bot::keyboards::user_menu())
            .await?;
            notify_auto_approve(
                bot,
                state,
                tg_user_id,
                tg_username,
                tg_display_name,
                &consumed,
            )
            .await;
            unmark_user_waiting_for_invite(state, tg_user_id).await;
        }
    }

    Ok(())
}

pub async fn send_user_link(
    bot: &Bot,
    chat_id: ChatId,
    tg_user_id: i64,
    state: &BotState,
) -> HandlerResult {
    let maybe = state.db.get_approved(tg_user_id).await?;
    match maybe {
        Some((_, secret)) => {
            let params = state.telemt_cfg.read_link_params()?;
            let link = build_proxy_link(&params, &secret)?;
            bot.send_message(chat_id, format!("Ваша ссылка на прокси:\n\n{}", link))
                .reply_markup(crate::bot::keyboards::user_menu())
                .await?;
        }
        None => {
            bot.send_message(
                chat_id,
                "У вас нет доступа к прокси. Отправьте /start для регистрации.",
            )
            .reply_markup(crate::bot::keyboards::user_menu())
            .await?;
        }
    }
    Ok(())
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

pub async fn perform_hard_ban(state: &BotState, tg_user_id: i64) -> Result<String, anyhow::Error> {
    let telemt_user = telemt_username(tg_user_id);
    let removed_from_cfg = state.telemt_cfg.remove_user(&telemt_user)?;
    if removed_from_cfg {
        reload_telemt_after_config_change(state);
    }
    let removed_from_db = state.db.deactivate_user(tg_user_id).await?;

    if removed_from_cfg || removed_from_db {
        Ok(format!("Пользователь {} удалён", telemt_user))
    } else {
        Ok(format!("Пользователь {} не найден", telemt_user))
    }
}

pub async fn admin_show_pending(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    let pending = state.db.list_pending_requests(10).await?;
    if pending.is_empty() {
        bot.send_message(chat_id, "Новых заявок нет.")
            .reply_markup(crate::bot::keyboards::admin_menu())
            .await?;
        return Ok(());
    }

    bot.send_message(chat_id, format!("Найдено новых заявок: {}", pending.len()))
        .reply_markup(crate::bot::keyboards::admin_menu())
        .await?;

    for req in pending {
        let text = format!(
            "📋 Заявка #{}:\n\
             User ID: {}\n\
             Username: @{}\n\
             Имя: {}\n\
             Время: {}",
            req.id,
            req.tg_user_id,
            req.tg_username.as_deref().unwrap_or("—"),
            req.tg_display_name.as_deref().unwrap_or("—"),
            format_timestamp(req.created_at),
        );
        bot.send_message(chat_id, text)
            .reply_markup(crate::bot::keyboards::approve_reject_buttons(req.id))
            .await?;
    }
    Ok(())
}

pub async fn admin_show_users_page(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    requested_page: i64,
    message_id: Option<teloxide::types::MessageId>,
) -> HandlerResult {
    let total_users = state.db.count_active_users().await?;
    let users_page_size = state.config.users_page_size.max(1);
    if total_users <= 0 {
        let text = "Активных пользователей нет.";
        if let Some(message_id) = message_id {
            bot.edit_message_text(chat_id, message_id, text)
                .reply_markup(InlineKeyboardMarkup::default())
                .await?;
        } else {
            bot.send_message(chat_id, text)
                .reply_markup(crate::bot::keyboards::admin_menu())
                .await?;
        }
        return Ok(());
    }

    let total_pages = ((total_users + users_page_size - 1) / users_page_size).max(1);
    let page = requested_page.clamp(1, total_pages);
    let offset = (page - 1) * users_page_size;
    let users = state
        .db
        .list_active_users_page(users_page_size, offset)
        .await?;

    let titles: Vec<(i64, String)> = users
        .iter()
        .map(|user| {
            let display_name = user_display_name(user);
            let short = if display_name.chars().count() > 40 {
                format!("{}...", display_name.chars().take(37).collect::<String>())
            } else {
                display_name
            };
            (user.tg_user_id, format!("{} (id {})", short, user.tg_user_id))
        })
        .collect();

    let text = format!(
        "👥 Активные пользователи\nВсего: {}\nСтраница: {}/{}\n\nНажмите на пользователя, чтобы открыть карточку.",
        total_users, page, total_pages
    );
    let keyboard = crate::bot::keyboards::users_page_keyboard(&titles, page, total_pages);

    if let Some(message_id) = message_id {
        bot.edit_message_text(chat_id, message_id, text)
            .reply_markup(keyboard)
            .await?;
    } else {
        bot.send_message(chat_id, text).reply_markup(keyboard).await?;
    }
    Ok(())
}

pub async fn admin_show_stats(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    let stats = state.db.admin_stats().await?;
    let text = format!(
        "📊 Статистика:\n\
         Всего записей: {}\n\
         Ожидают: {}\n\
         Активные: {}\n\
         Отклонённые: {}\n\
         Удалённые: {}",
        stats.total, stats.pending, stats.approved, stats.rejected, stats.deleted
    );
    bot.send_message(chat_id, text)
        .reply_markup(crate::bot::keyboards::admin_menu())
        .await?;
    Ok(())
}

pub async fn admin_show_service_panel(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    let result = state.service.status();
    let text = format!(
        "⚙️ Сервис telemt\n\n{}",
        state.service.format_result("status", &result)
    );
    bot.send_message(chat_id, text)
        .reply_markup(crate::bot::keyboards::service_control_buttons())
        .await?;
    Ok(())
}

pub async fn send_user_qr_to_admin(
    bot: &Bot,
    q: &CallbackQuery,
    user: &RegistrationRequest,
    state: &BotState,
) -> Result<(), anyhow::Error> {
    let Some(secret) = user.secret.as_deref() else {
        return Err(anyhow!("Не найден секрет пользователя"));
    };

    let params = state.telemt_cfg.read_link_params()?;
    let link = build_proxy_link(&params, secret)?;
    let qr_png = build_user_qr_png_bytes(&link)?;
    let caption = super::format::render_user_proxy_for_forward(user, &link);

    if let Some((chat_id, _)) = callback_message_target(q) {
        bot.send_photo(
            chat_id,
            InputFile::memory(qr_png).file_name(format!("telemt-proxy-{}.png", user.tg_user_id)),
        )
        .caption(caption)
        .await?;
    }
    Ok(())
}

pub fn callback_prefix_filter(prefix: &'static str) -> impl Fn(CallbackQuery) -> Option<CallbackQuery> {
    move |q: CallbackQuery| {
        if q.data.as_deref().is_some_and(|payload| payload.starts_with(prefix)) {
            Some(q)
        } else {
            None
        }
    }
}

pub fn user_id_or_reply(msg: &Message) -> Result<i64, anyhow::Error> {
    sender_user_id(msg).ok_or_else(|| anyhow!("Не удалось определить пользователя отправителя"))
}
