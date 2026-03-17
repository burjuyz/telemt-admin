use super::callback_data::ServiceAction;
use super::format::{
    format_timestamp, render_invite_token_button_title, render_invite_token_card_text,
    render_user_card_text, usage_guide_text, user_display_name,
};
use super::shared::{HandlerResult, build_user_qr_png_bytes, callback_message_target};
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

fn page_bounds(total_items: i64, page_size: i64, requested_page: i64) -> (i64, i64, i64) {
    let total_pages = ((total_items + page_size - 1) / page_size).max(1);
    let page = requested_page.clamp(1, total_pages);
    let offset = (page - 1) * page_size;
    (page, total_pages, offset)
}

fn compact_line(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        trimmed.to_string()
    } else {
        format!(
            "{}...",
            trimmed
                .chars()
                .take(limit.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn service_action_title(action: ServiceAction) -> &'static str {
    match action {
        ServiceAction::Start => "запустить сервис",
        ServiceAction::Stop => "остановить сервис",
        ServiceAction::Restart => "перезапустить сервис",
        ServiceAction::Reload => "перечитать конфиг",
        ServiceAction::Status => "обновить статус",
    }
}

async fn render_service_panel_text(
    state: &BotState,
    notice: Option<&str>,
) -> Result<String, anyhow::Error> {
    let summary = state.service.summary();
    let service_events = state.service.recent_events(3);
    let admin_events = state.db.list_recent_admin_activities(4).await?;
    let stats = state.db.admin_stats().await?;
    let active_tokens = state.db.count_active_invite_tokens().await?;

    let status_label = match (summary.active_state.as_str(), summary.sub_state.as_str()) {
        ("active", "running") => "работает",
        ("active", value) => value,
        ("inactive", value) => value,
        (value, _) => value,
    };

    let mut lines = vec![format!(
        "⚙️ Сервис {}\nСтатус: {}",
        state.service.service_name(),
        status_label
    )];
    lines.push(format!(
        "Проверка: {}",
        if summary.success {
            "OK"
        } else {
            "Ошибка"
        }
    ));

    if let Some(notice) = notice {
        lines.push(format!("Действие: {}", notice));
    }

    lines.push(format!(
        "Unit: {} | PID: {}",
        summary.unit_file_state,
        summary
            .main_pid
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "—".to_string())
    ));
    lines.push(format!(
        "Пользователи: {} | Заявки: {} | Токены: {}",
        stats.approved, stats.pending, active_tokens
    ));

    if let Some(exec_status) = summary.exec_main_status {
        lines.push(format!("Код процесса: {}", exec_status));
    }
    if let Some(error) = &summary.error {
        lines.push(format!("Ошибка статуса: {}", compact_line(error, 90)));
    }

    lines.push(String::new());
    lines.push("События сервиса:".to_string());
    if service_events.lines.is_empty() {
        lines.push(
            service_events
                .error
                .as_deref()
                .map(|error| format!("• {}", compact_line(error, 90)))
                .unwrap_or_else(|| "• нет данных".to_string()),
        );
    } else {
        if !service_events.success {
            lines.push("• журнал прочитан частично".to_string());
        }
        for line in service_events.lines.iter().take(3) {
            lines.push(format!("• {}", compact_line(line, 90)));
        }
    }

    lines.push(String::new());
    lines.push("Действия админа:".to_string());
    if admin_events.is_empty() {
        lines.push("• пока нет событий".to_string());
    } else {
        for item in admin_events.iter().take(4) {
            lines.push(format!(
                "• {} · {}",
                format_timestamp(item.timestamp),
                compact_line(&item.summary, 70)
            ));
        }
    }

    Ok(lines.join("\n"))
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
    let text = "Панель администратора\n\nВыберите раздел ниже.";
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
                "Доступ уже открыт.\n\nНажмите «Получить ссылку».".to_string()
            }
            RequestStatus::Pending => {
                "Заявка уже на рассмотрении.\n\nДождитесь решения администратора.".to_string()
            }
            RequestStatus::Rejected => {
                "Заявка отклонена.\n\nЕсли есть новый invite-токен, отправьте /start и введите его заново.".to_string()
            }
            RequestStatus::Deleted => {
                "Доступ был отозван.\n\nДля новой регистрации отправьте /start и введите invite-токен заново.".to_string()
            }
        }
    } else {
        "Чтобы получить доступ, отправьте /start и введите invite-токен.\n\nЕсли токен уже есть, нажмите кнопку ниже."
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
    let text = "Управление invite-токенами\n\nВыберите действие.";
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text.to_string(),
        crate::bot::keyboards::token_menu_keyboard(state.config.security.allow_auto_approve_tokens),
    )
    .await
}

pub async fn admin_show_token_list_page(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    requested_page: i64,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let total_tokens = state.db.count_active_invite_tokens().await?;
    let tokens_page_size = state.config.users_page_size.max(1);
    if total_tokens <= 0 {
        upsert_screen(
            bot,
            chat_id,
            message_id,
            "🎟 Токены\n\nАктивных invite-токенов нет.".to_string(),
            crate::bot::keyboards::token_menu_keyboard(
                state.config.security.allow_auto_approve_tokens,
            ),
        )
        .await?;
        return Ok(());
    }

    let (page, total_pages, offset) = page_bounds(total_tokens, tokens_page_size, requested_page);
    let tokens = state
        .db
        .list_active_invite_tokens_page(tokens_page_size, offset)
        .await?;

    let items: Vec<(i64, String)> = tokens
        .iter()
        .map(|token| (token.id, render_invite_token_button_title(token)))
        .collect();
    let text = format!(
        "🎟 Токены · {}\nСтраница: {}/{}\n\nВыберите токен.",
        total_tokens, page, total_pages
    );
    let keyboard = crate::bot::keyboards::token_list_keyboard(&items, page, total_pages);
    upsert_screen(bot, chat_id, message_id, text, keyboard).await?;
    Ok(())
}

pub async fn show_token_card(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    token: &crate::db::InviteToken,
    page: i64,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        render_invite_token_card_text(token),
        crate::bot::keyboards::token_card_keyboard(token.id, page),
    )
    .await
}

pub async fn show_token_revoke_confirm(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    token: &crate::db::InviteToken,
    page: i64,
) -> HandlerResult {
    bot.edit_message_text(
        chat_id,
        message_id,
        format!(
            "Отозвать invite-токен {}?\n\nПосле этого его нельзя будет использовать для регистрации.",
            token.token
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_token_revoke_keyboard(
        token.id, page,
    ))
    .await?;
    Ok(())
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

pub async fn admin_show_pending_requests_page(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    requested_page: i64,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let total_pending = state.db.count_pending_requests().await?;
    let requests_page_size = state.config.users_page_size.max(1);
    if total_pending <= 0 {
        upsert_screen(
            bot,
            chat_id,
            message_id,
            "📥 Заявки\n\nНовых заявок нет.".to_string(),
            crate::bot::keyboards::admin_home_keyboard(),
        )
        .await?;
        return Ok(());
    }

    let (page, total_pages, offset) = page_bounds(total_pending, requests_page_size, requested_page);
    let pending = state
        .db
        .list_pending_requests_page(requests_page_size, offset)
        .await?;
    let items: Vec<(i64, String)> = pending
        .iter()
        .map(|req| {
            (
                req.id,
                format!("📋 #{} · {}", req.id, user_display_name(req)),
            )
        })
        .collect();
    let text = format!(
        "📥 Заявки · {}\nСтраница: {}/{}\n\nВыберите заявку.",
        total_pending, page, total_pages
    );
    let keyboard = crate::bot::keyboards::pending_requests_keyboard(&items, page, total_pages);
    upsert_screen(bot, chat_id, message_id, text, keyboard).await?;
    Ok(())
}

pub async fn show_pending_request_card(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    request: &crate::db::RegistrationRequest,
    page: i64,
) -> HandlerResult {
    let text = format!(
        "📋 Заявка #{}\n\n\
         👤 {}\n\
         🆔 {}\n\
         📱 {}\n\
         📅 {}",
        request.id,
        user_display_name(request),
        request.tg_user_id,
        request
            .tg_username
            .as_deref()
            .map(|username| format!("@{}", username))
            .unwrap_or_else(|| "—".to_string()),
        format_timestamp(request.created_at),
    );
    bot.edit_message_text(chat_id, message_id, text)
        .reply_markup(crate::bot::keyboards::pending_request_card_keyboard(
            request.id, page,
        ))
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
        upsert_screen(
            bot,
            chat_id,
            message_id,
            "👥 Пользователи\n\nАктивных пользователей нет.\n\nМожно создать нового пользователя."
                .to_string(),
            crate::bot::keyboards::users_page_keyboard(&[], 1, 1),
        )
        .await?;
        return Ok(());
    }

    let (page, total_pages, offset) = page_bounds(total_users, users_page_size, requested_page);
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
            (
                user.tg_user_id,
                format!("{} (id {})", short, user.tg_user_id),
            )
        })
        .collect();

    let text = format!(
        "👥 Пользователи · {}\nСтраница: {}/{}\n\nВыберите пользователя.",
        total_users, page, total_pages
    );
    let keyboard = crate::bot::keyboards::users_page_keyboard(&titles, page, total_pages);
    upsert_screen(bot, chat_id, message_id, text, keyboard).await?;
    Ok(())
}

pub async fn admin_show_stats(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let stats = state.db.admin_stats().await?;
    let text = format!(
        "📊 Статистика\n\n\
         Всего: {}\n\
         Заявки: {}\n\
         Активные: {}\n\
         Отклонённые: {}\n\
         Удалённые: {}",
        stats.total, stats.pending, stats.approved, stats.rejected, stats.deleted
    );
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::stats_keyboard(),
    )
    .await
}

pub async fn admin_show_service_panel(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    message_id: Option<MessageId>,
) -> HandlerResult {
    admin_show_service_panel_with_notice(bot, chat_id, state, message_id, None).await
}

pub async fn admin_show_service_panel_with_notice(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    message_id: Option<MessageId>,
    notice: Option<&str>,
) -> HandlerResult {
    let text = render_service_panel_text(state, notice).await?;
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::service_control_buttons(),
    )
    .await
}

pub async fn show_service_action_confirm(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    action: ServiceAction,
) -> HandlerResult {
    bot.edit_message_text(
        chat_id,
        message_id,
        format!(
            "Подтвердить действие: {}?\n\nЭто может временно прервать доступ пользователей.",
            service_action_title(action)
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_service_action_keyboard(
        action,
    ))
    .await?;
    Ok(())
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
    let caption = format!(
        "👤 {} ({})\n\n🔗 {}",
        user_display_name(user),
        user.tg_user_id,
        link
    );

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
    message_id: Option<MessageId>,
    user: &crate::db::RegistrationRequest,
    page: i64,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        render_user_card_text(user),
        crate::bot::keyboards::user_card_keyboard(user.tg_user_id, page),
    )
    .await
}
