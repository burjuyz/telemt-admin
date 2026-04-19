use super::super::common::{ack_callback, admin_callback_target, replace_wizard_state, start_wizard_from_callback};
use super::AdminActionResult;
use crate::bot::handlers::actions::send_token_start_link;
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{
    admin_show_token_list_page, show_token_card, show_token_menu, show_token_revoke_confirm,
};
use crate::bot::handlers::shared::{callback_message_target, require_admin_callback};
use crate::bot::handlers::state::{BotState, WizardState};
use crate::bot::keyboards;
use teloxide::payloads::EditMessageTextSetters;
use teloxide::prelude::{Bot, CallbackQuery, Requester};

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
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptTokenCreate { auto_approve },
                "Жду параметры токена",
                "Введите срок доступа пользователя в днях:\n\
                 • 30 дней\n\
                 • 60 дней\n\
                 • 180 дней\n\
                 • или другое число (1-365)".to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::SetTokenExpiration { days, auto_approve } => {
            let Some((admin_id, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some(&format!("Срок: {} дн.", days)), false).await?;
            
            let new_state = WizardState::AdminTokenAwaitingMaxIps {
                auto_approve,
                expiration_days: Some(days),
            };
            replace_wizard_state(state, admin_id, new_state).await?;
            
            let keyboard = keyboards::token_max_ips_keyboard(auto_approve, days);
            bot.edit_message_text(
                chat_id,
                message_id,
                format!(
                    "Срок доступа: {} дн.\n\n\
                     Теперь выберите лимит IP (Max Unique IPs):",
                    days
                ),
            )
            .reply_markup(keyboard)
            .await?;
            Ok(true)
        }
        CallbackAction::SetTokenMaxIps { count, auto_approve, expiration_days } => {
            let Some((admin_id, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let ip_text = count.map(|c| c.to_string()).unwrap_or_else(|| "без лимита".to_string());
            ack_callback(bot, q.id.clone(), Some(&format!("IP: {}", ip_text)), false).await?;
            
            let new_state = WizardState::AdminTokenAwaitingDataQuota {
                auto_approve,
                expiration_days: Some(expiration_days),
                max_unique_ips: count,
            };
            replace_wizard_state(state, admin_id, new_state).await?;
            
            bot.edit_message_text(
                chat_id,
                message_id,
                format!(
                    "Срок доступа: {} дн.\nЛимит IP: {}\n\n\
                     Теперь выберите квоту трафика:",
                    expiration_days,
                    ip_text
                ),
            )
            .reply_markup(keyboards::token_data_quota_keyboard(auto_approve, expiration_days, count))
            .await?;
            Ok(true)
        }
        CallbackAction::SetTokenDataQuota { quota_gb, auto_approve, expiration_days, max_unique_ips } => {
            let Some((admin_id, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let quota_text = match quota_gb {
                Some(0) => "безлимит".to_string(),
                Some(gb) => format!("{} GB", gb),
                None => "другое...".to_string(),
            };
            ack_callback(bot, q.id.clone(), Some(&format!("Квота: {}", quota_text)), false).await?;
            
            let data_quota_bytes = quota_gb.map(|gb| gb * 1024 * 1024 * 1024);
            
            let new_state = WizardState::AdminTokenAwaitingGroup {
                auto_approve,
                expiration_days,
                max_unique_ips,
                data_quota_bytes,
            };
            replace_wizard_state(state, admin_id, new_state).await?;
            
            bot.edit_message_text(
                chat_id,
                message_id,
                format!(
                    "Срок доступа: {} дн.\nЛимит IP: {}\nКвота: {}\n\n\
                     Введите ID группы для токена (или 0 для без группы):",
                    expiration_days,
                    max_unique_ips.map(|i| i.to_string()).unwrap_or_else(|| "—".to_string()),
                    quota_text
                ),
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
        CallbackAction::PromptEditTokenGroup { token_id, page } => {
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptEditTokenGroup { token_id, page },
                "Жду ID группы",
                "Введите ID группы для токена:\n\
                 • ID группы (например, 1, 2, 3)\n\
                 • 0 — убрать группу\n\n\
                 Текущую группу токена можно посмотреть в его карточке.".to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::SendTokenStartLink { token_id } => {
            let Some((_, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Отправляю ссылку"), false).await?;
            send_token_start_link(bot, chat_id, state, token_id).await?;
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
