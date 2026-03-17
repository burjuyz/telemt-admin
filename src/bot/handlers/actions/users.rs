use super::access::approve_user_direct_and_build_link;
use crate::bot::handlers::screens::{show_delete_user_confirm, show_user_card};
use crate::bot::handlers::shared::{parse_create_target, CreateTarget};
use crate::bot::handlers::state::{telemt_username, BotState};
use teloxide::prelude::{Bot, ChatId, Requester};

enum UserLookupTarget {
    UserId(i64),
    Username(String),
}

fn resolve_user_lookup_target(arg: &str) -> Option<UserLookupTarget> {
    let target = parse_create_target(arg)?;

    match target {
        CreateTarget::UserId(id) => Some(UserLookupTarget::UserId(id)),
        CreateTarget::Username(username) => Some(UserLookupTarget::Username(username)),
    }
}

pub async fn create_user_from_input(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    arg: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let tg_user_id: i64 = match resolve_user_lookup_target(arg) {
        Some(UserLookupTarget::UserId(id)) => id,
        Some(UserLookupTarget::Username(username)) => {
            match state.db.find_tg_user_id_by_username(&username).await? {
                Some(id) => id,
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
            bot.send_message(chat_id, "Использование: ID или @username")
                .await?;
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
    state: &BotState,
    arg: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    match arg.trim().parse::<i64>() {
        Ok(tg_user_id) => {
            if state
                .db
                .get_active_user_by_tg_user(tg_user_id)
                .await?
                .is_none()
            {
                bot.send_message(
                    chat_id,
                    format!("Активный пользователь с Telegram ID {} не найден.", tg_user_id),
                )
                .await?;
                return Ok(false);
            }
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

pub async fn open_user_from_lookup_input(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    arg: &str,
    page: i64,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let tg_user_id = match resolve_user_lookup_target(arg) {
        Some(UserLookupTarget::UserId(id)) => id,
        Some(UserLookupTarget::Username(username)) => {
            match state.db.find_tg_user_id_by_username(&username).await? {
                Some(id) => id,
                None => {
                    bot.send_message(chat_id, format!("Пользователь @{} не найден в базе.", username))
                        .await?;
                    return Ok(false);
                }
            }
        }
        None => {
            bot.send_message(chat_id, "Укажите Telegram ID или @username одним сообщением.")
                .await?;
            return Ok(false);
        }
    };

    let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
        bot.send_message(chat_id, format!("Активный пользователь {} не найден.", tg_user_id))
            .await?;
        return Ok(false);
    };

    show_user_card(bot, chat_id, None, &user, page).await?;
    Ok(true)
}

pub async fn has_active_users(
    state: &BotState,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    Ok(state.db.count_active_users().await? > 0)
}
