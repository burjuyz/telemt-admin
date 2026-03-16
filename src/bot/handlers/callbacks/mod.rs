use super::callback_data::{CallbackAction, ServiceAction};
use super::commands::has_active_users;
use super::screens::{
    admin_show_service_panel, admin_show_stats, admin_show_token_list_page, admin_show_users_page,
    send_user_qr_to_admin, show_admin_home, show_pending_request_card, show_pending_requests,
    show_token_card, show_token_list, show_token_menu, show_token_revoke_confirm, show_usage_guide,
    show_user_ban_confirm, show_user_card, show_user_home,
};
use super::shared::{
    HandlerResult, approve_request_and_build_link, callback_message_target, perform_hard_ban,
    require_admin_callback, send_user_link,
};
use super::state::{BotState, WizardState, clear_wizard_state, set_wizard_state};
use teloxide::prelude::*;

pub fn handler()
-> teloxide::dispatching::UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    Update::filter_callback_query().endpoint(handle_callback)
}

async fn handle_callback(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    let Some(data) = q.data.as_deref() else {
        return Ok(());
    };
    let Some(action) = CallbackAction::decode(data) else {
        bot.answer_callback_query(q.id.clone())
            .text("Устаревшая или некорректная кнопка")
            .show_alert(true)
            .await?;
        return Ok(());
    };

    match action {
        CallbackAction::ShowAdminHome => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                clear_wizard_state(&state, q.from.id.0 as i64).await?;
                bot.answer_callback_query(q.id.clone()).await?;
                show_admin_home(&bot, chat_id, Some(message_id)).await?;
            }
        }
        CallbackAction::ShowUserHome => {
            let user_id = q.from.id.0 as i64;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                clear_wizard_state(&state, user_id).await?;
                bot.answer_callback_query(q.id.clone()).await?;
                show_user_home(&bot, chat_id, Some(message_id), &state, user_id).await?;
            }
        }
        CallbackAction::ShowUserLink => {
            let user_id = q.from.id.0 as i64;
            let username = q.from.username.as_deref();
            let display_name = Some(q.from.full_name());
            bot.answer_callback_query(q.id.clone())
                .text("Отправляю ссылку")
                .await?;
            if let Some((chat_id, _)) = callback_message_target(&q) {
                send_user_link(
                    &bot,
                    chat_id,
                    user_id,
                    username,
                    display_name.as_deref(),
                    &state,
                )
                .await?;
            }
        }
        CallbackAction::ShowUsageGuide => {
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                bot.answer_callback_query(q.id.clone()).await?;
                show_usage_guide(&bot, chat_id, Some(message_id)).await?;
            }
        }
        CallbackAction::PromptInviteToken => {
            let user_id = q.from.id.0 as i64;
            clear_wizard_state(&state, user_id).await?;
            set_wizard_state(&state, user_id, WizardState::AwaitingInviteToken).await?;
            bot.answer_callback_query(q.id.clone())
                .text("Жду invite-токен следующим сообщением")
                .await?;
            if let Some((chat_id, _)) = callback_message_target(&q) {
                bot.send_message(
                    chat_id,
                    "Отправьте invite-токен следующим сообщением.\n\nСообщение с кнопками можно оставить открытым.",
                )
                .await?;
            }
        }
        CallbackAction::CancelWizard => {
            let user_id = q.from.id.0 as i64;
            clear_wizard_state(&state, user_id).await?;
            bot.answer_callback_query(q.id.clone())
                .text("Сценарий отменён")
                .await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                if state.config.is_admin(user_id) {
                    show_admin_home(&bot, chat_id, Some(message_id)).await?;
                } else {
                    show_user_home(&bot, chat_id, Some(message_id), &state, user_id).await?;
                }
            }
        }
        CallbackAction::ShowPendingRequests => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            bot.answer_callback_query(q.id.clone()).await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                show_pending_requests(&bot, chat_id, Some(message_id), &state).await?;
            }
        }
        CallbackAction::OpenPendingRequest { request_id } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            let Some((chat_id, message_id)) = callback_message_target(&q) else {
                return Ok(());
            };
            let Some(request) = state.db.get_pending_by_id(request_id).await? else {
                bot.answer_callback_query(q.id.clone())
                    .text("Заявка уже обработана")
                    .await?;
                show_pending_requests(&bot, chat_id, Some(message_id), &state).await?;
                return Ok(());
            };
            bot.answer_callback_query(q.id.clone())
                .text("Открыта заявка")
                .await?;
            show_pending_request_card(&bot, chat_id, message_id, &request).await?;
        }
        CallbackAction::ShowUsersPage { page } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            bot.answer_callback_query(q.id.clone()).await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                admin_show_users_page(&bot, chat_id, &state, page, Some(message_id)).await?;
            }
        }
        CallbackAction::OpenUserCard { tg_user_id, page } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                bot.answer_callback_query(q.id.clone())
                    .text("Пользователь уже неактивен")
                    .show_alert(true)
                    .await?;
                return Ok(());
            };
            bot.answer_callback_query(q.id.clone())
                .text("Открыта карточка")
                .await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                show_user_card(&bot, chat_id, message_id, &user, page).await?;
            }
        }
        CallbackAction::ViewUserQr { tg_user_id } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                bot.answer_callback_query(q.id.clone())
                    .text("Пользователь уже неактивен")
                    .show_alert(true)
                    .await?;
                return Ok(());
            };
            bot.answer_callback_query(q.id.clone())
                .text("Отправляю ссылку и QR")
                .await?;
            send_user_qr_to_admin(&bot, &q, &user, &state).await?;
        }
        CallbackAction::ConfirmUserBan { tg_user_id, page } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            bot.answer_callback_query(q.id.clone()).await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                show_user_ban_confirm(&bot, chat_id, message_id, tg_user_id, page).await?;
            }
        }
        CallbackAction::ExecuteUserBan { tg_user_id, page } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            let status_text = perform_hard_ban(&state, tg_user_id).await?;
            bot.answer_callback_query(q.id.clone())
                .text(status_text.clone())
                .await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                bot.send_message(chat_id, status_text).await?;
                admin_show_users_page(&bot, chat_id, &state, page, Some(message_id)).await?;
            }
        }
        CallbackAction::ShowStats => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            bot.answer_callback_query(q.id.clone()).await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                admin_show_stats(&bot, chat_id, &state, Some(message_id)).await?;
            }
        }
        CallbackAction::ShowServicePanel => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            bot.answer_callback_query(q.id.clone()).await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                admin_show_service_panel(&bot, chat_id, &state, Some(message_id)).await?;
            }
        }
        CallbackAction::RunServiceAction { action } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            let (action_name, result) = match action {
                ServiceAction::Start => ("start", state.service.start()),
                ServiceAction::Stop => ("stop", state.service.stop()),
                ServiceAction::Restart => ("restart", state.service.restart()),
                ServiceAction::Reload => ("reload", state.service.reload()),
                ServiceAction::Status => ("status", state.service.status()),
            };
            bot.answer_callback_query(q.id.clone())
                .text(format!("Выполнено: {}", action_name))
                .await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                let text = format!(
                    "⚙️ Сервис telemt\n\n{}",
                    state.service.format_result(action_name, &result)
                );
                bot.edit_message_text(chat_id, message_id, text)
                    .reply_markup(crate::bot::keyboards::service_control_buttons())
                    .await?;
            }
        }
        CallbackAction::ShowTokenMenu => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            bot.answer_callback_query(q.id.clone()).await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                show_token_menu(&bot, chat_id, Some(message_id), &state).await?;
            }
        }
        CallbackAction::PromptTokenCreate { auto_approve } => {
            let Some(admin_id) = require_admin_callback(&bot, &q, &state).await? else {
                return Ok(());
            };
            let security = &state.config.security;
            clear_wizard_state(&state, admin_id).await?;
            set_wizard_state(
                &state,
                admin_id,
                WizardState::AdminTokenCreateAwaitingParams { auto_approve },
            )
            .await?;
            bot.answer_callback_query(q.id.clone())
                .text("Жду параметры токена")
                .await?;
            if let Some((chat_id, _)) = callback_message_target(&q) {
                let format_hint = format!(
                    "Формат: одно число (дни) или два числа: дни и лимит использований.\n\
                     По умолчанию: {} дней, лимит без ограничений.\n\
                     Примеры: 7 или 7 3.",
                    security.default_token_days
                );
                let text = if auto_approve {
                    format!(
                        "Отправьте параметры авто-токена следующим сообщением.\n\n{}",
                        format_hint
                    )
                } else {
                    format!(
                        "Отправьте параметры токена следующим сообщением.\n\n{}",
                        format_hint
                    )
                };
                bot.send_message(chat_id, text).await?;
            }
        }
        CallbackAction::ShowTokenList => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            bot.answer_callback_query(q.id.clone()).await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                show_token_list(&bot, chat_id, Some(message_id), &state).await?;
            }
        }
        CallbackAction::ShowTokenListPage { page } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            bot.answer_callback_query(q.id.clone()).await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                admin_show_token_list_page(&bot, chat_id, &state, page, Some(message_id)).await?;
            }
        }
        CallbackAction::OpenTokenCard { token_id, page } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            let Some((chat_id, message_id)) = callback_message_target(&q) else {
                return Ok(());
            };
            let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? else {
                bot.answer_callback_query(q.id.clone())
                    .text("Токен уже недоступен")
                    .show_alert(true)
                    .await?;
                admin_show_token_list_page(&bot, chat_id, &state, page, Some(message_id)).await?;
                return Ok(());
            };
            bot.answer_callback_query(q.id.clone())
                .text("Открыта карточка токена")
                .await?;
            show_token_card(&bot, chat_id, message_id, &token, page).await?;
        }
        CallbackAction::ConfirmTokenRevoke { token_id, page } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            let Some((chat_id, message_id)) = callback_message_target(&q) else {
                return Ok(());
            };
            let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? else {
                bot.answer_callback_query(q.id.clone())
                    .text("Токен уже недоступен")
                    .show_alert(true)
                    .await?;
                admin_show_token_list_page(&bot, chat_id, &state, page, Some(message_id)).await?;
                return Ok(());
            };
            bot.answer_callback_query(q.id.clone()).await?;
            show_token_revoke_confirm(&bot, chat_id, message_id, &token, page).await?;
        }
        CallbackAction::ExecuteTokenRevoke { token_id, page } => {
            let Some(admin_id) = require_admin_callback(&bot, &q, &state).await? else {
                return Ok(());
            };
            let revoked = state.db.revoke_invite_token_by_id(token_id).await?;
            let status_text = if revoked {
                tracing::info!("Admin {} revoked invite token #{}", admin_id, token_id);
                "Токен отозван".to_string()
            } else {
                "Токен не найден или уже недоступен".to_string()
            };
            bot.answer_callback_query(q.id.clone())
                .text(status_text.clone())
                .await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                admin_show_token_list_page(&bot, chat_id, &state, page, Some(message_id)).await?;
            }
        }
        CallbackAction::PromptTokenRevoke => {
            let Some(admin_id) = require_admin_callback(&bot, &q, &state).await? else {
                return Ok(());
            };
            clear_wizard_state(&state, admin_id).await?;
            set_wizard_state(&state, admin_id, WizardState::AdminTokenRevokeAwaitingToken).await?;
            bot.answer_callback_query(q.id.clone())
                .text("Жду код токена")
                .await?;
            if let Some((chat_id, _)) = callback_message_target(&q) {
                bot.send_message(
                    chat_id,
                    "Отправьте код токена следующим сообщением.\n\nСообщение с кнопками можно оставить открытым.",
                )
                .await?;
            }
        }
        CallbackAction::PromptCreateUser => {
            let Some(admin_id) = require_admin_callback(&bot, &q, &state).await? else {
                return Ok(());
            };
            clear_wizard_state(&state, admin_id).await?;
            set_wizard_state(&state, admin_id, WizardState::AdminCreateAwaitingTarget).await?;
            bot.answer_callback_query(q.id.clone())
                .text("Жду ID или @username")
                .await?;
            if let Some((chat_id, _)) = callback_message_target(&q) {
                bot.send_message(
                    chat_id,
                    "Отправьте Telegram ID или @username следующим сообщением.\n\nСообщение с кнопками можно оставить открытым.",
                )
                .await?;
            }
        }
        CallbackAction::PromptDeleteUser => {
            let Some(admin_id) = require_admin_callback(&bot, &q, &state).await? else {
                return Ok(());
            };
            if !has_active_users(&state).await? {
                clear_wizard_state(&state, admin_id).await?;
                bot.answer_callback_query(q.id.clone())
                    .text("Активных пользователей нет")
                    .show_alert(true)
                    .await?;
                return Ok(());
            }
            clear_wizard_state(&state, admin_id).await?;
            set_wizard_state(&state, admin_id, WizardState::AdminDeleteAwaitingTarget).await?;
            bot.answer_callback_query(q.id.clone())
                .text("Жду Telegram ID")
                .await?;
            if let Some((chat_id, _)) = callback_message_target(&q) {
                bot.send_message(
                    chat_id,
                    "Отправьте Telegram ID пользователя следующим сообщением.\n\nСообщение с кнопками можно оставить открытым.",
                )
                .await?;
            }
        }
        CallbackAction::ConfirmDeleteUser { tg_user_id } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            bot.answer_callback_query(q.id.clone()).await?;
            if let Some((chat_id, _)) = callback_message_target(&q) {
                super::screens::show_delete_user_confirm(&bot, chat_id, tg_user_id).await?;
            }
        }
        CallbackAction::ExecuteDeleteUser { tg_user_id } => {
            if require_admin_callback(&bot, &q, &state).await?.is_none() {
                return Ok(());
            }
            let status_text = perform_hard_ban(&state, tg_user_id).await?;
            bot.answer_callback_query(q.id.clone())
                .text(status_text.clone())
                .await?;
            if let Some((chat_id, message_id)) = callback_message_target(&q) {
                bot.edit_message_text(chat_id, message_id, status_text)
                    .reply_markup(crate::bot::keyboards::admin_home_keyboard())
                    .await?;
            }
        }
        CallbackAction::ApproveRequest { request_id } => {
            let Some(admin_id) = require_admin_callback(&bot, &q, &state).await? else {
                return Ok(());
            };
            let message_target = callback_message_target(&q);
            let (request, link) = match approve_request_and_build_link(&state, request_id).await? {
                Some(payload) => payload,
                None => {
                    bot.answer_callback_query(q.id.clone())
                        .text("Заявка уже обработана или не найдена")
                        .await?;
                    return Ok(());
                }
            };
            bot.answer_callback_query(q.id.clone())
                .text("Одобрено")
                .await?;
            if let Some((chat_id, message_id)) = message_target {
                bot.edit_message_text(chat_id, message_id, "✅ Заявка одобрена")
                    .reply_markup(crate::bot::keyboards::pending_result_keyboard())
                    .await?;
            }
            bot.send_message(
                ChatId(request.tg_user_id),
                format!("Ваша ссылка на прокси:\n\n{}", link),
            )
            .await?;
            tracing::info!("Admin {} approved request #{}", admin_id, request_id);
        }
        CallbackAction::RejectRequest { request_id } => {
            let Some(admin_id) = require_admin_callback(&bot, &q, &state).await? else {
                return Ok(());
            };
            let message_target = callback_message_target(&q);
            let request = state.db.reject(request_id).await?;
            bot.answer_callback_query(q.id.clone())
                .text("Отклонено")
                .await?;
            if let Some(request) = request {
                if let Some((chat_id, message_id)) = message_target {
                    bot.edit_message_text(chat_id, message_id, "❌ Заявка отклонена")
                        .reply_markup(crate::bot::keyboards::pending_result_keyboard())
                        .await?;
                }
                bot.send_message(
                    ChatId(request.tg_user_id),
                    "Ваша заявка на регистрацию отклонена администратором.",
                )
                .await?;
            }
            tracing::info!("Admin {} rejected request #{}", admin_id, request_id);
        }
    }
    Ok(())
}
