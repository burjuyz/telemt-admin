use super::super::common::{ack_callback, admin_callback_target, start_wizard_from_callback};
use super::AdminActionResult;
use crate::bot::handlers::actions::{has_active_users, perform_hard_ban};
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{
    admin_show_users_page, send_user_qr_to_admin, show_user_ban_confirm, show_user_card,
};
use crate::bot::handlers::shared::{callback_message_target, require_admin_callback};
use crate::bot::handlers::state::{BotState, clear_wizard_state};
use teloxide::payloads::EditMessageTextSetters;
use teloxide::prelude::{Bot, CallbackQuery, Requester};

pub async fn handle(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> AdminActionResult {
    match action {
        CallbackAction::ShowUsersPage { page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_users_page(bot, chat_id, state, page, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::PromptUserLookup { page } => {
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptUserLookup { page },
                "Жду Telegram ID или @username",
                "Отправьте Telegram ID или @username следующим сообщением.\n\nСписок можно оставить открытым.".to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::OpenUserCard { tg_user_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Открыта карточка"), false).await?;
            show_user_card(bot, chat_id, Some(message_id), &user, page).await?;
            Ok(true)
        }
        CallbackAction::ViewUserQr { tg_user_id } => {
            let Some(_) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Отправляю ссылку и QR"), false).await?;
            send_user_qr_to_admin(bot, q, &user, state).await?;
            Ok(true)
        }
        CallbackAction::ConfirmUserBan { tg_user_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_user_ban_confirm(bot, chat_id, message_id, tg_user_id, page).await?;
            Ok(true)
        }
        CallbackAction::ExecuteUserBan { tg_user_id, page } => {
            let Some(_) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let status_text = perform_hard_ban(state, tg_user_id).await?;
            ack_callback(bot, q.id.clone(), Some(&status_text), false).await?;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                bot.send_message(chat_id, status_text).await?;
                admin_show_users_page(bot, chat_id, state, page, Some(message_id)).await?;
            }
            Ok(true)
        }
        CallbackAction::PromptCreateUser => {
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptCreateUser,
                "Жду ID или @username",
                "Отправьте Telegram ID или @username следующим сообщением.\n\nСообщение с кнопками можно оставить открытым.".to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::PromptDeleteUser => {
            let Some(admin_id) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            if !has_active_users(state).await? {
                clear_wizard_state(state, admin_id).await?;
                ack_callback(bot, q.id.clone(), Some("Активных пользователей нет"), true).await?;
                return Ok(true);
            }
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptDeleteUser,
                "Жду Telegram ID",
                "Отправьте Telegram ID пользователя следующим сообщением.\n\nСообщение с кнопками можно оставить открытым.".to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::ExecuteDeleteUser { tg_user_id } => {
            let Some(_) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let status_text = perform_hard_ban(state, tg_user_id).await?;
            ack_callback(bot, q.id.clone(), Some(&status_text), false).await?;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                bot.edit_message_text(chat_id, message_id, status_text)
                    .reply_markup(crate::bot::keyboards::admin_home_keyboard())
                    .await?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
