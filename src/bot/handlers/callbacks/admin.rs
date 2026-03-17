use super::common::{ack_callback, admin_callback_target, start_wizard_from_callback};
use crate::bot::handlers::actions::{
    approve_request_and_build_link, execute_service_action, has_active_users, perform_hard_ban,
};
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{
    admin_show_pending_requests_page, admin_show_service_panel, admin_show_service_panel_with_notice,
    admin_show_stats, admin_show_token_list_page, admin_show_users_page, send_user_qr_to_admin,
    show_admin_home, show_pending_request_card, show_service_action_confirm, show_token_card,
    show_token_menu, show_token_revoke_confirm, show_user_ban_confirm, show_user_card,
};
use crate::bot::handlers::shared::{callback_message_target, require_admin_callback};
use teloxide::payloads::EditMessageTextSetters;
use crate::bot::handlers::state::{clear_wizard_state, BotState};
use teloxide::prelude::{Bot, CallbackQuery, ChatId, Requester};

pub async fn handle_admin_action(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    match action {
        CallbackAction::ShowAdminHome => {
            let Some((admin_id, chat_id, message_id)) = admin_callback_target(bot, q, state).await?
            else {
                return Ok(true);
            };
            clear_wizard_state(state, admin_id).await?;
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_admin_home(bot, chat_id, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ShowPendingRequests => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_pending_requests_page(bot, chat_id, state, 1, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ShowPendingRequestsPage { page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_pending_requests_page(bot, chat_id, state, page, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::OpenPendingRequest { request_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(request) = state.db.get_pending_by_id(request_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Заявка уже обработана"), false).await?;
                admin_show_pending_requests_page(bot, chat_id, state, page, Some(message_id)).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Открыта заявка"), false).await?;
            show_pending_request_card(bot, chat_id, message_id, &request, page).await?;
            Ok(true)
        }
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
        CallbackAction::ShowStats => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_stats(bot, chat_id, state, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ShowServicePanel => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_service_panel(bot, chat_id, state, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ConfirmServiceAction { action } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_service_action_confirm(bot, chat_id, message_id, action).await?;
            Ok(true)
        }
        CallbackAction::ExecuteServiceAction { action } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let status_text = execute_service_action(state, action);
            ack_callback(bot, q.id.clone(), Some(&status_text), false).await?;
            admin_show_service_panel_with_notice(
                bot,
                chat_id,
                state,
                Some(message_id),
                Some(&status_text),
            )
            .await?;
            Ok(true)
        }
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
        CallbackAction::ApproveRequest { request_id, page } => {
            let Some(admin_id) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let message_target = callback_message_target(q);
            let (request, link) = match approve_request_and_build_link(state, request_id).await? {
                Some(payload) => payload,
                None => {
                    ack_callback(
                        bot,
                        q.id.clone(),
                        Some("Заявка уже обработана или не найдена"),
                        false,
                    )
                    .await?;
                    return Ok(true);
                }
            };
            ack_callback(bot, q.id.clone(), Some("Одобрено"), false).await?;
            if let Some((chat_id, message_id)) = message_target {
                bot.edit_message_text(chat_id, message_id, "✅ Заявка одобрена")
                    .reply_markup(crate::bot::keyboards::pending_result_keyboard(page))
                    .await?;
            }
            bot.send_message(ChatId(request.tg_user_id), format!("Ваша ссылка на прокси:\n\n{}", link))
                .await?;
            tracing::info!("Admin {} approved request #{}", admin_id, request_id);
            Ok(true)
        }
        CallbackAction::RejectRequest { request_id, page } => {
            let Some(admin_id) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let message_target = callback_message_target(q);
            let request = state.db.reject(request_id).await?;
            ack_callback(bot, q.id.clone(), Some("Отклонено"), false).await?;
            if let Some(request) = request {
                if let Some((chat_id, message_id)) = message_target {
                    bot.edit_message_text(chat_id, message_id, "❌ Заявка отклонена")
                        .reply_markup(crate::bot::keyboards::pending_result_keyboard(page))
                        .await?;
                }
                bot.send_message(
                    ChatId(request.tg_user_id),
                    "Ваша заявка на регистрацию отклонена администратором.",
                )
                .await?;
            }
            tracing::info!("Admin {} rejected request #{}", admin_id, request_id);
            Ok(true)
        }
        _ => Ok(false),
    }
}
