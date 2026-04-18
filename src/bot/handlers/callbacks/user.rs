use super::common::{ack_callback, replace_wizard_state};
use crate::bot::handlers::actions::try_auto_import_remote_user_by_tg_id;
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::keyboards;
use crate::bot::handlers::screens::{
    show_admin_home, show_user_home, show_user_link_screen, show_usage_guide,
};
use crate::bot::handlers::shared::callback_message_target;
use crate::bot::handlers::state::{BotState, WizardState, clear_wizard_state};
use teloxide::prelude::{Bot, CallbackQuery, Requester};

pub async fn handle_user_action(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    match action {
        CallbackAction::ShowUserHome => {
            let user_id = q.from.id.0 as i64;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                clear_wizard_state(state, user_id).await?;
                let username = q.from.username.as_deref();
                let display_name = q.from.full_name();
                let _ = try_auto_import_remote_user_by_tg_id(
                    state,
                    user_id,
                    username,
                    Some(display_name.as_str()),
                    None,
                )
                .await?;
                ack_callback(bot, q.id.clone(), None, false).await?;
                show_user_home(bot, chat_id, Some(message_id), state, user_id).await?;
            }
            Ok(true)
        }
        CallbackAction::ShowUserLink => {
            let user_id = q.from.id.0 as i64;
            let username = q.from.username.as_deref();
            let display_name = Some(q.from.full_name());
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                ack_callback(bot, q.id.clone(), None, false).await?;
                let _ = try_auto_import_remote_user_by_tg_id(
                    state,
                    user_id,
                    username,
                    display_name.as_deref(),
                    None,
                )
                .await?;
                show_user_link_screen(bot, chat_id, Some(message_id), state, user_id).await?;
            }
            Ok(true)
        }
        CallbackAction::ShowUsageGuide => {
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                ack_callback(bot, q.id.clone(), None, false).await?;
                show_usage_guide(bot, chat_id, Some(message_id)).await?;
            }
            Ok(true)
        }
        CallbackAction::PromptInviteToken => {
            let user_id = q.from.id.0 as i64;
            replace_wizard_state(state, user_id, WizardState::AwaitingInviteToken).await?;
            ack_callback(
                bot,
                q.id.clone(),
                Some("Жду invite-токен следующим сообщением"),
                false,
            )
            .await?;
            if let Some((chat_id, _)) = callback_message_target(q) {
                bot.send_message(
                    chat_id,
                    state.config.bot_messages.invite_followup_prompt_or_default(),
                )
                .await?;
            }
            Ok(true)
        }
        CallbackAction::CancelWizard => {
            let user_id = q.from.id.0 as i64;
            clear_wizard_state(state, user_id).await?;
            ack_callback(bot, q.id.clone(), Some("Сценарий отменён"), false).await?;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                if state.config.is_admin(user_id) {
                    show_admin_home(bot, chat_id, Some(message_id)).await?;
                } else {
                    let username = q.from.username.as_deref();
                    let display_name = q.from.full_name();
                    let _ = try_auto_import_remote_user_by_tg_id(
                        state,
                        user_id,
                        username,
                        Some(display_name.as_str()),
                        None,
                    )
                    .await?;
                    show_user_home(bot, chat_id, Some(message_id), state, user_id).await?;
                }
            }
            Ok(true)
        }
        CallbackAction::BackTokenWizard => {
            let user_id = q.from.id.0 as i64;
            let state_key = state.db.get_wizard_state(user_id).await?;
            let current_state = WizardState::decode(state_key.as_deref().unwrap_or(""));
            let (prev_auto_approve, new_state) = match current_state {
                Some(WizardState::AdminTokenAwaitingMaxIps { auto_approve, .. }) => {
                    (auto_approve, WizardState::AdminTokenAwaitingExpiration { auto_approve })
                }
                Some(WizardState::AdminTokenAwaitingDataQuota { auto_approve, expiration_days, max_unique_ips }) => {
                    (auto_approve, WizardState::AdminTokenAwaitingMaxIps { auto_approve, expiration_days })
                }
                _ => {
                    clear_wizard_state(state, user_id).await?;
                    ack_callback(bot, q.id.clone(), Some("Нет предыдущего шага"), false).await?;
                    return Ok(true);
                }
            };
            set_wizard_state(state, user_id, new_state).await?;
            let step_text = match current_state {
                Some(WizardState::AdminTokenAwaitingMaxIps { auto_approve, .. }) => {
                    "Выберите срок доступа пользователя в днях:\n• 30 дней\n• 60 дней\n• 180 дней\n• или другое число (1-365)"
                }
                Some(WizardState::AdminTokenAwaitingDataQuota { .. }) => {
                    "Введите лимит IP (количество устройств, с которых можно подключаться).\nНапример: 3 или пропустите отправив /skip"
                }
                _ => "Назад"
            };
            ack_callback(bot, q.id.clone(), Some("Шаг назад"), false).await?;
            if let Some((chat_id, _)) = callback_message_target(q) {
                bot.send_message(chat_id, step_text)
                    .reply_markup(keyboards::cancel_keyboard(CallbackAction::BackTokenWizard))
                    .await?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
