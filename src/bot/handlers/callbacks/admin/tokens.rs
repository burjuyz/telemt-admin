use super::super::common::{ack_callback, admin_callback_target, start_wizard_from_callback};
use super::AdminActionResult;
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{
    admin_show_token_list_page, show_token_card, show_token_menu, show_token_revoke_confirm,
};
use crate::bot::handlers::shared::{callback_message_target, require_admin_callback};
use crate::bot::handlers::state::BotState;
use teloxide::prelude::{Bot, CallbackQuery};

pub async fn handle(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> AdminActionResult {
    match action {
        CallbackAction::ShowTokenMenu => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_token_menu(bot, chat_id, Some(message_id), state).await?;
            Ok(true)
        }
        CallbackAction::PromptTokenCreate { auto_approve } => {
            let format_hint = format!(
                "Формат: одно число (дни) или два числа: дни и лимит использований.\n\
                 По умолчанию: {} дней, лимит без ограничений.\n\
                 Примеры: 7 или 7 3.",
                state.config.security.default_token_days
            );
            let prompt_text = if auto_approve {
                format!(
                    "Отправьте параметры авто-токена следующим сообщением.\n\n{}",
                    format_hint
                )
            } else {
                format!("Отправьте параметры токена следующим сообщением.\n\n{}", format_hint)
            };
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptTokenCreate { auto_approve },
                "Жду параметры токена",
                prompt_text,
            )
            .await?;
            Ok(true)
        }
        CallbackAction::ShowTokenList => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_token_list_page(bot, chat_id, state, 1, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ShowTokenListPage { page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_token_list_page(bot, chat_id, state, page, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::PromptTokenLookup { page } => {
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptTokenLookup { page },
                "Жду код токена",
                "Отправьте код токена следующим сообщением.\n\nСписок можно оставить открытым."
                    .to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::OpenTokenCard { token_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Токен уже недоступен"), true).await?;
                admin_show_token_list_page(bot, chat_id, state, page, Some(message_id)).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Открыта карточка токена"), false).await?;
            show_token_card(bot, chat_id, Some(message_id), &token, page).await?;
            Ok(true)
        }
        CallbackAction::ConfirmTokenRevoke { token_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Токен уже недоступен"), true).await?;
                admin_show_token_list_page(bot, chat_id, state, page, Some(message_id)).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_token_revoke_confirm(bot, chat_id, message_id, &token, page).await?;
            Ok(true)
        }
        CallbackAction::ExecuteTokenRevoke { token_id, page } => {
            let Some(admin_id) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let revoked = state.db.revoke_invite_token_by_id(token_id).await?;
            let status_text = if revoked {
                tracing::info!("Admin {} revoked invite token #{}", admin_id, token_id);
                "Токен отозван".to_string()
            } else {
                "Токен не найден или уже недоступен".to_string()
            };
            ack_callback(bot, q.id.clone(), Some(&status_text), false).await?;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                admin_show_token_list_page(bot, chat_id, state, page, Some(message_id)).await?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
