use super::format::{format_timestamp, render_user_card_text, usage_guide_text, user_display_name};
use super::shared::{build_user_qr_png_bytes, callback_message_target, HandlerResult};
use super::state::BotState;
use crate::db::RequestStatus;
use crate::link::build_proxy_link;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, InputFile, KeyboardRemove, MessageId};

async fn upsert_screen(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    text: String,
    reply_markup: InlineKeyboardMarkup,
) -> HandlerResult {
    if let Some(message_id) = message_id {
        bot.edit_message_text(chat_id, message_id, text)
            .reply_markup(reply_markup)
            .await?;
    } else {
        bot.send_message(chat_id, text)
            .reply_markup(reply_markup)
            .await?;
    }
    Ok(())
}

pub async fn send_text_with_keyboard_removed(
    bot: &Bot,
    chat_id: ChatId,
    text: impl Into<String>,
) -> HandlerResult {
    bot.send_message(chat_id, text.into())
        .reply_markup(KeyboardRemove::new())
        .await?;
    Ok(())
}

pub async fn show_admin_home(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let text = "Панель администратора\n\n\
Используйте slash-команды как точки входа:\n\
/service, /token, /create, /delete, /help\n\n\
Или выберите нужный раздел ниже.";
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text.to_string(),
        crate::bot::keyboards::admin_home_keyboard(),
    )
    .await
}

pub async fn show_user_home(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
    user_id: i64,
) -> HandlerResult {
    let text = if let Some(existing) = state.db.get_request_by_tg_user(user_id).await? {
        match existing.status {
            RequestStatus::Approved => {
                "Доступ уже одобрен.\n\nИспользуйте /link, чтобы получить ссылку на прокси.".to_string()
            }
            RequestStatus::Pending => {
                "Ваша заявка уже на рассмотрении.\n\nДождитесь решения администратора."
                    .to_string()
            }
            RequestStatus::Rejected => {
                "Ваша заявка была отклонена.\n\nЕсли у вас есть новый invite-токен, отправьте /start и введите его заново.".to_string()
            }
            RequestStatus::Deleted => {
                "Доступ был отозван.\n\nДля новой регистрации отправьте /start и введите invite-токен заново.".to_string()
            }
        }
    } else {
        "Чтобы получить доступ, отправьте /start и введите invite-токен следующим сообщением.\n\n\
Если токен уже есть, можете нажать кнопку ниже."
            .to_string()
    };

    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::user_home_keyboard(),
    )
    .await
}

pub async fn show_usage_guide(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        usage_guide_text().to_string(),
        crate::bot::keyboards::guide_keyboard(),
    )
    .await
}

pub async fn show_token_menu(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
) -> HandlerResult {
    let text = "Управление invite-токенами\n\n\
Выберите действие:\n\
- создать новый токен;\n\
- посмотреть активные;\n\
- отозвать существующий.";
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text.to_string(),
        crate::bot::keyboards::token_menu_keyboard(state.config.security.allow_auto_approve_tokens),
    )
    .await
}

pub async fn show_delete_user_confirm(
    bot: &Bot,
    chat_id: ChatId,
    tg_user_id: i64,
) -> HandlerResult {
    bot.send_message(
        chat_id,
        format!(
            "Удалить пользователя с Telegram ID {}?\n\nДействие деактивирует пользователя в БД и удалит его из telemt-конфига.",
            tg_user_id
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_delete_keyboard(tg_user_id))
    .await?;
    Ok(())
}

pub async fn show_user_ban_confirm(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    tg_user_id: i64,
    page: i64,
) -> HandlerResult {
    bot.edit_message_text(
        chat_id,
        message_id,
        format!(
            "Удалить пользователя {}?\n\nЭто действие уберёт запись из telemt и деактивирует доступ в БД.",
            tg_user_id
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_user_ban_keyboard(
        tg_user_id, page,
    ))
    .await?;
    Ok(())
}

pub async fn admin_show_pending(bot: &Bot, chat_id: ChatId, state: &BotState) -> HandlerResult {
    let pending = state.db.list_pending_requests(10).await?;
    if pending.is_empty() {
        bot.send_message(chat_id, "Новых заявок нет.")
            .reply_markup(crate::bot::keyboards::admin_home_keyboard())
            .await?;
        return Ok(());
    }

    bot.send_message(chat_id, format!("Найдено новых заявок: {}", pending.len()))
        .reply_markup(crate::bot::keyboards::admin_home_keyboard())
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
    message_id: Option<MessageId>,
) -> HandlerResult {
    let total_users = state.db.count_active_users().await?;
    let users_page_size = state.config.users_page_size.max(1);
    if total_users <= 0 {
        let text = "Активных пользователей нет.".to_string();
        let keyboard = crate::bot::keyboards::admin_home_keyboard();
        if let Some(message_id) = message_id {
            bot.edit_message_text(chat_id, message_id, text)
                .reply_markup(keyboard)
                .await?;
        } else {
            bot.send_message(chat_id, text).reply_markup(keyboard).await?;
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
        .reply_markup(crate::bot::keyboards::admin_home_keyboard())
        .await?;
    Ok(())
}

pub async fn admin_show_service_panel(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let result = state.service.status();
    let text = format!(
        "⚙️ Сервис telemt\n\n{}",
        state.service.format_result("status", &result)
    );
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::service_control_buttons(),
    )
    .await
}

pub async fn send_user_qr_to_admin(
    bot: &Bot,
    q: &CallbackQuery,
    user: &crate::db::RegistrationRequest,
    state: &BotState,
) -> Result<(), anyhow::Error> {
    let Some(secret) = user.secret.as_deref() else {
        return Err(anyhow::anyhow!("Не найден секрет пользователя"));
    };

    let params = state.telemt_cfg.read_link_params()?;
    let link = build_proxy_link(&params, secret)?;
    let qr_png = build_user_qr_png_bytes(&link)?;
    let caption = format!("👤 {} ({})\n\n🔗 {}", user_display_name(user), user.tg_user_id, link);

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

pub async fn show_user_card(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    user: &crate::db::RegistrationRequest,
    page: i64,
) -> HandlerResult {
    bot.edit_message_text(chat_id, message_id, render_user_card_text(user))
        .reply_markup(crate::bot::keyboards::user_card_keyboard(
            user.tg_user_id,
            page,
        ))
        .await?;
    Ok(())
}
