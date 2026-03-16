use super::callback_data::CallbackAction;
use super::format::{format_date, format_mode, render_invite_token_line};
use super::screens::{
    admin_show_service_panel, show_admin_home, show_delete_user_confirm, show_token_menu,
    show_user_home, send_text_with_keyboard_removed,
};
use super::shared::{
    approve_request_and_build_link, approve_user_direct_and_build_link, build_bot_start_link,
    parse_create_target, parse_start_token, process_invite_token, send_user_link, user_id_or_reply,
    CreateTarget, HandlerResult,
};
use super::state::{
    clear_wizard_state, is_admin_message, sender_display_name, sender_user_id, set_wizard_state,
    telemt_username, BotState, WizardState,
};
use crate::db::RequestStatus;
use teloxide::dptree;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use teloxide::utils::command::BotCommands;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum BotCommand {
    #[command(description = "Главный экран / регистрация")]
    Start,
    #[command(description = "Получить ссылку на прокси")]
    Link,
    #[command(description = "Справка")]
    Help,
    #[command(description = "Одобрить заявку (админ)")]
    Approve,
    #[command(description = "Отклонить заявку (админ)")]
    Reject,
    #[command(description = "Создать пользователя (админ)")]
    Create,
    #[command(description = "Удалить пользователя (админ)")]
    Delete,
    #[command(description = "Управление сервисом (админ)")]
    Service,
    #[command(description = "Управление invite-токенами (админ)")]
    Token,
}

pub fn telegram_commands() -> Vec<teloxide::types::BotCommand> {
    BotCommand::bot_commands()
}

pub fn handler() -> teloxide::dispatching::UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    dptree::filter(|msg: Message| {
        msg.text()
            .is_some_and(|text| text.trim_start().starts_with('/'))
    })
    .endpoint(handle_command_message)
}

fn extract_command_name<'a>(text: &'a str, bot_username: Option<&str>) -> Option<&'a str> {
    let command = text.split_whitespace().next()?.strip_prefix('/')?;
    let (name, mentioned_bot) = command.split_once('@').map_or((command, None), |(name, bot)| {
        (name, Some(bot))
    });
    if let (Some(mentioned_bot), Some(bot_username)) = (mentioned_bot, bot_username) {
        let bot_username = bot_username.trim_start_matches('@');
        if !mentioned_bot.eq_ignore_ascii_case(bot_username) {
            return None;
        }
    }
    Some(name)
}

async fn handle_command_message(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let Some(command_name) = extract_command_name(text, state.bot_username.as_deref()) else {
        return Ok(());
    };

    match command_name {
        "start" => start_cmd(bot, msg, state).await,
        "link" => cmd_link(bot, msg, state).await,
        "help" => cmd_help(bot, msg, state).await,
        "approve" => cmd_approve(bot, msg, state).await,
        "reject" => cmd_reject(bot, msg, state).await,
        "create" => cmd_create(bot, msg, state).await,
        "delete" => cmd_delete(bot, msg, state).await,
        "service" => cmd_service(bot, msg, state).await,
        "token" => cmd_token(bot, msg, state).await,
        _ => {
            bot.send_message(msg.chat.id, "Неизвестная команда. Используйте /help.")
                .await?;
            Ok(())
        }
    }
}

pub async fn cmd_help(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(user_id) = sender_user_id(&msg) else {
        return Ok(());
    };
    let is_admin = state.config.is_admin(user_id);
    let text = if is_admin {
        r#"Команды:
/start — открыть главный экран администратора
/help — показать эту справку
/service — открыть панель управления сервисом
/token — открыть мастер invite-токенов
/create — запустить создание пользователя
/delete — запустить удаление пользователя
/approve <id> — быстро одобрить заявку
/reject <id> — быстро отклонить заявку

Подсказка:
- сложные сценарии запускаются через slash-команды и продолжаются inline-кнопками;
- старые аргументы `/service ...` и `/token ...` по-прежнему поддерживаются."#
    } else {
        r#"Команды:
/start — начать регистрацию или открыть главный экран
/link — получить ссылку на прокси
/help — показать справку

Дальше бот сам подскажет, какой шаг нужен: ввести invite-токен, дождаться одобрения или забрать ссылку."#
    };
    send_text_with_keyboard_removed(&bot, msg.chat.id, text).await?;
    Ok(())
}

async fn start_cmd(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let user_id = match user_id_or_reply(&msg) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(error = %error, "Received /start without sender");
            return Ok(());
        }
    };
    let username = msg.from.as_ref().and_then(|u| u.username.clone());
    let display_name = sender_display_name(&msg);
    tracing::info!(
        user_id = user_id,
        username = ?username,
        display_name = ?display_name,
        "Received /start command"
    );

    if state.config.is_admin(user_id) {
        clear_wizard_state(&state, user_id).await?;
        send_text_with_keyboard_removed(
            &bot,
            msg.chat.id,
            "Постоянное меню отключено. Используйте slash-команды и inline-кнопки.",
        )
        .await?;
        show_admin_home(&bot, msg.chat.id, None).await?;
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    if let Some(token) = parse_start_token(text) {
        process_invite_token(
            &bot,
            &msg,
            &state,
            user_id,
            username.as_deref(),
            display_name.as_deref(),
            &token,
        )
        .await?;
        return Ok(());
    }

    send_text_with_keyboard_removed(
        &bot,
        msg.chat.id,
        "Постоянное меню отключено. Используйте slash-команды и inline-кнопки.",
    )
    .await?;

    if let Some(existing) = state.db.get_request_by_tg_user(user_id).await? {
        clear_wizard_state(&state, user_id).await?;
        match existing.status {
            RequestStatus::Approved | RequestStatus::Pending | RequestStatus::Rejected => {
                show_user_home(&bot, msg.chat.id, None, &state, user_id).await?;
                return Ok(());
            }
            RequestStatus::Deleted => {}
        }
    }

    set_wizard_state(&state, user_id, WizardState::AwaitingInviteToken).await?;
    bot.send_message(
        msg.chat.id,
        "Введите пригласительный токен следующим сообщением.\n\nЕсли передумали, нажмите «Отмена».",
    )
    .reply_markup(crate::bot::keyboards::cancel_keyboard(
        CallbackAction::ShowUserHome,
    ))
    .await?;
    Ok(())
}

async fn cmd_link(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(user_id) = sender_user_id(&msg) else {
        return Ok(());
    };
    let username = msg.from.as_ref().and_then(|u| u.username.as_deref());
    let display_name = sender_display_name(&msg);
    tracing::info!(user_id = user_id, "Received /link command");

    send_user_link(
        &bot,
        msg.chat.id,
        user_id,
        username,
        display_name.as_deref(),
        &state,
    )
    .await
}

async fn cmd_approve(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let request_id: i64 = match text.split_whitespace().nth(1).unwrap_or("").parse() {
        Ok(id) => id,
        Err(_) => {
            bot.send_message(msg.chat.id, "Использование: /approve <request_id>")
                .await?;
            return Ok(());
        }
    };
    tracing::info!(request_id = request_id, "Admin command /approve");

    let (request, link) = match approve_request_and_build_link(&state, request_id).await? {
        Some(payload) => payload,
        None => {
            bot.send_message(msg.chat.id, "Заявка не найдена или уже обработана")
                .await?;
            return Ok(());
        }
    };

    bot.send_message(
        msg.chat.id,
        format!("Одобрено. Ссылка отправлена пользователю.\n{}", link),
    )
    .await?;
    bot.send_message(
        ChatId(request.tg_user_id),
        format!("Ваша ссылка на прокси:\n\n{}", link),
    )
    .await?;
    Ok(())
}

async fn cmd_reject(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let request_id: i64 = match text.split_whitespace().nth(1).unwrap_or("").parse() {
        Ok(id) => id,
        Err(_) => {
            bot.send_message(msg.chat.id, "Использование: /reject <request_id>")
                .await?;
            return Ok(());
        }
    };
    tracing::info!(request_id = request_id, "Admin command /reject");

    let req = state.db.reject(request_id).await?;
    if let Some(r) = req {
        bot.send_message(msg.chat.id, "Заявка отклонена").await?;
        bot.send_message(
            ChatId(r.tg_user_id),
            "Ваша заявка на регистрацию отклонена администратором.",
        )
        .await?;
    } else {
        bot.send_message(msg.chat.id, "Заявка не найдена или уже обработана")
            .await?;
    }
    Ok(())
}

async fn cmd_create(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let arg = text.split_whitespace().nth(1).unwrap_or("");
    let user_id = sender_user_id(&msg).unwrap_or_default();
    if arg.is_empty() {
        clear_wizard_state(&state, user_id).await?;
        set_wizard_state(&state, user_id, WizardState::AdminCreateAwaitingTarget).await?;
        bot.send_message(
            msg.chat.id,
            "Отправьте Telegram ID или @username следующим сообщением.\n\n\
Для варианта с @username пользователь должен раньше написать боту /start.",
        )
        .reply_markup(crate::bot::keyboards::cancel_keyboard(
            CallbackAction::ShowAdminHome,
        ))
        .await?;
        return Ok(());
    }

    let _ = create_user_from_input(&bot, msg.chat.id, &state, arg).await?;
    Ok(())
}

async fn cmd_delete(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let user_id = sender_user_id(&msg).unwrap_or_default();
    match text.split_whitespace().nth(1) {
        Some(arg) => match arg.parse::<i64>() {
            Ok(tg_user_id) => show_delete_user_confirm(&bot, msg.chat.id, tg_user_id).await?,
            Err(_) => {
                bot.send_message(msg.chat.id, "Использование: /delete <telegram_user_id>")
                    .await?;
            }
        },
        None => {
            clear_wizard_state(&state, user_id).await?;
            set_wizard_state(&state, user_id, WizardState::AdminDeleteAwaitingTarget).await?;
            bot.send_message(
                msg.chat.id,
                "Отправьте Telegram ID пользователя, которого нужно удалить.",
            )
            .reply_markup(crate::bot::keyboards::cancel_keyboard(
                CallbackAction::ShowAdminHome,
            ))
            .await?;
        }
    }
    Ok(())
}

async fn cmd_service(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let args: Vec<&str> = text.split_whitespace().collect();
    if let Some(action) = args.get(1).copied() {
        tracing::info!(action = action, "Admin command /service legacy action");
        let (action_name, result) = match action {
            "start" => ("start", state.service.start()),
            "stop" => ("stop", state.service.stop()),
            "restart" => ("restart", state.service.restart()),
            "reload" => ("reload", state.service.reload()),
            "status" => ("status", state.service.status()),
            _ => {
                bot.send_message(
                    msg.chat.id,
                    "Использование: /service <start|stop|status|reload|restart>",
                )
                .await?;
                return Ok(());
            }
        };

        let reply = state.service.format_result(action_name, &result);
        bot.send_message(msg.chat.id, reply).await?;
        return Ok(());
    }

    admin_show_service_panel(&bot, msg.chat.id, &state, None).await?;
    Ok(())
}

async fn cmd_token(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    if !is_admin_message(&msg, &state) {
        return Ok(());
    }

    let text = msg.text().unwrap_or("");
    let args: Vec<&str> = text.split_whitespace().collect();
    if args.len() == 1 {
        show_token_menu(&bot, msg.chat.id, None, &state).await?;
        return Ok(());
    }

    let Some(subcommand) = args.get(1).copied() else {
        show_token_menu(&bot, msg.chat.id, None, &state).await?;
        return Ok(());
    };

    match subcommand {
        "create" => {
            let mut days: Option<i64> = None;
            let mut auto_approve = false;
            let mut max_uses: Option<i64> = None;
            let mut index = 2;

            while index < args.len() {
                match args[index] {
                    "--auto" | "-a" => {
                        auto_approve = true;
                        index += 1;
                    }
                    "--max-uses" => {
                        let Some(value) = args.get(index + 1) else {
                            bot.send_message(
                                msg.chat.id,
                                "Использование: /token create [days] [--auto|-a] [--max-uses N]",
                            )
                            .await?;
                            return Ok(());
                        };
                        let parsed = match value.parse::<i64>() {
                            Ok(parsed) if parsed >= 1 => parsed,
                            _ => {
                                bot.send_message(
                                    msg.chat.id,
                                    "Параметр --max-uses должен быть целым числом >= 1.",
                                )
                                .await?;
                                return Ok(());
                            }
                        };
                        max_uses = Some(parsed);
                        index += 2;
                    }
                    value => {
                        if let Ok(parsed_days) = value.parse::<i64>() {
                            if days.is_some() {
                                bot.send_message(
                                    msg.chat.id,
                                    "Использование: /token create [days] [--auto|-a] [--max-uses N]",
                                )
                                .await?;
                                return Ok(());
                            }
                            days = Some(parsed_days);
                            index += 1;
                            continue;
                        }
                        bot.send_message(
                            msg.chat.id,
                            "Использование: /token create [days] [--auto|-a] [--max-uses N]",
                        )
                        .await?;
                        return Ok(());
                    }
                }
            }

            let security = &state.config.security;
            let days = days.unwrap_or(security.default_token_days);
            if days < 1 {
                bot.send_message(msg.chat.id, "Срок действия должен быть не меньше 1 дня.")
                    .await?;
                return Ok(());
            }
            if days > security.max_token_days {
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Нельзя создать токен на срок больше {} дней.",
                        security.max_token_days
                    ),
                )
                .await?;
                return Ok(());
            }
            if auto_approve && !security.allow_auto_approve_tokens {
                bot.send_message(
                    msg.chat.id,
                    "Автоподтверждение токенов запрещено в конфигурации.",
                )
                .await?;
                return Ok(());
            }

            let created_by = sender_user_id(&msg);
            let token = state
                .db
                .create_invite_token(days, auto_approve, max_uses, created_by)
                .await?;

            let link_line = state
                .bot_username
                .as_deref()
                .map(|bot_username| {
                    let invite_link = build_bot_start_link(bot_username, &token.token);
                    format!("Ссылка: {}\n", invite_link)
                })
                .unwrap_or_else(|| {
                    "Ссылка: недоступна (у бота не задан username в Telegram).\n".to_string()
                });

            let response = format!(
                "✅ Токен создан:\n\
                 Код: <code>{}</code>\n\
                 {}\
                 Режим: {}\n\
                 Действует до: {}\n\
                 Лимит использований: {}\n\
                 Используйте команду <code>/token revoke {}</code> для отзыва.",
                token.token,
                link_line,
                format_mode(token.auto_approve),
                format_date(token.expires_at),
                token
                    .max_usage
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "без лимита".to_string()),
                token.token
            );
            bot.send_message(msg.chat.id, response)
                .parse_mode(ParseMode::Html)
                .await?;
        }
        "list" => {
            let tokens = state.db.list_active_invite_tokens(50).await?;
            if tokens.is_empty() {
                bot.send_message(msg.chat.id, "Активных invite-токенов нет.")
                    .await?;
                return Ok(());
            }

            let mut lines: Vec<String> = Vec::with_capacity(tokens.len());
            for token in tokens {
                lines.push(render_invite_token_line(&token));
            }
            let text = format!("Активные токены:\n\n{}", lines.join("\n"));
            bot.send_message(msg.chat.id, text).await?;
        }
        "revoke" => {
            let Some(token_value) = args.get(2).copied() else {
                bot.send_message(msg.chat.id, "Использование: /token revoke <token>")
                    .await?;
                return Ok(());
            };
            let revoked = state.db.revoke_invite_token(token_value).await?;
            if revoked {
                bot.send_message(msg.chat.id, format!("Токен {} отозван.", token_value))
                    .await?;
            } else {
                bot.send_message(msg.chat.id, "Токен не найден или уже отозван.")
                    .await?;
            }
        }
        _ => {
            bot.send_message(
                msg.chat.id,
                "Использование:\n/token create [days] [--auto|-a] [--max-uses N]\n/token list\n/token revoke <token>",
            )
            .await?;
        }
    }

    Ok(())
}

pub async fn create_user_from_input(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    arg: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let tg_user_id: i64 = match parse_create_target(arg) {
        Some(CreateTarget::UserId(id)) => id,
        Some(CreateTarget::Username(username)) => {
            match state.db.find_tg_user_id_by_username(&username).await? {
                Some(user_id) => user_id,
                None => {
                    bot.send_message(
                        chat_id,
                        format!(
                            "Пользователь @{} не найден в базе.\n\
                             Он должен хотя бы раз отправить боту /start.",
                            username
                        ),
                    )
                    .await?;
                    return Ok(false);
                }
            }
        }
        None => {
            bot.send_message(chat_id, "Использование: ID или @username").await?;
            return Ok(false);
        }
    };
    tracing::info!(tg_user_id = tg_user_id, "Admin create user");

    let telemt_user = telemt_username(tg_user_id);
    let link = approve_user_direct_and_build_link(state, tg_user_id, None, None).await?;

    bot.send_message(
        chat_id,
        format!("Пользователь {} создан.\nСсылка:\n{}", telemt_user, link),
    )
    .await?;
    Ok(true)
}

pub async fn prompt_delete_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    arg: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    match arg.trim().parse::<i64>() {
        Ok(tg_user_id) => {
            show_delete_user_confirm(bot, chat_id, tg_user_id).await?;
            Ok(true)
        }
        Err(_) => {
            bot.send_message(chat_id, "Нужен корректный Telegram ID пользователя.")
                .await?;
            Ok(false)
        }
    }
}

pub async fn handle_token_create_from_text(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    auto_approve: bool,
    text: &str,
    created_by: Option<i64>,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let security = &state.config.security;
    if auto_approve && !security.allow_auto_approve_tokens {
        bot.send_message(
            chat_id,
            "Автоподтверждение токенов запрещено в конфигурации.",
        )
        .await?;
        return Ok(false);
    }

    let mut days: Option<i64> = None;
    let mut max_uses: Option<i64> = None;
    let args: Vec<&str> = text.split_whitespace().collect();
    let mut index = 0;

    while index < args.len() {
        match args[index] {
            "--max-uses" => {
                let Some(value) = args.get(index + 1) else {
                    bot.send_message(
                        chat_id,
                        "Формат: [days] [--max-uses N]\nНапример: 7 --max-uses 3 или пустое сообщение для значений по умолчанию.",
                    )
                    .await?;
                    return Ok(false);
                };
                let parsed = match value.parse::<i64>() {
                    Ok(parsed) if parsed >= 1 => parsed,
                    _ => {
                        bot.send_message(chat_id, "Параметр --max-uses должен быть >= 1.")
                            .await?;
                        return Ok(false);
                    }
                };
                max_uses = Some(parsed);
                index += 2;
            }
            value => {
                if let Ok(parsed_days) = value.parse::<i64>() {
                    if days.is_some() {
                        bot.send_message(chat_id, "Срок действия можно указать только один раз.")
                            .await?;
                        return Ok(false);
                    }
                    days = Some(parsed_days);
                    index += 1;
                    continue;
                }
                bot.send_message(
                    chat_id,
                    "Формат: [days] [--max-uses N]\nНапример: 7 --max-uses 3.",
                )
                .await?;
                return Ok(false);
            }
        }
    }

    let days = days.unwrap_or(security.default_token_days);
    if days < 1 {
        bot.send_message(chat_id, "Срок действия должен быть не меньше 1 дня.")
            .await?;
        return Ok(false);
    }
    if days > security.max_token_days {
        bot.send_message(
            chat_id,
            format!(
                "Нельзя создать токен на срок больше {} дней.",
                security.max_token_days
            ),
        )
        .await?;
        return Ok(false);
    }

    let token = state
        .db
        .create_invite_token(days, auto_approve, max_uses, created_by)
        .await?;

    let link_line = state
        .bot_username
        .as_deref()
        .map(|bot_username| {
            let invite_link = build_bot_start_link(bot_username, &token.token);
            format!("Ссылка: {}\n", invite_link)
        })
        .unwrap_or_else(|| {
            "Ссылка: недоступна (у бота не задан username в Telegram).\n".to_string()
        });

    let response = format!(
        "✅ Токен создан:\n\
         Код: <code>{}</code>\n\
         {}\
         Режим: {}\n\
         Действует до: {}\n\
         Лимит использований: {}\n\
         Для отзыва используйте `/token` -> «Отозвать токен» или старую команду <code>/token revoke {}</code>.",
        token.token,
        link_line,
        format_mode(token.auto_approve),
        format_date(token.expires_at),
        token
            .max_usage
            .map(|value| value.to_string())
            .unwrap_or_else(|| "без лимита".to_string()),
        token.token
    );
    bot.send_message(chat_id, response)
        .parse_mode(ParseMode::Html)
        .await?;
    Ok(true)
}
