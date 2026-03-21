use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::shared::{
    HandlerResult, callback_message_target, require_admin_callback,
};
use crate::bot::handlers::state::{BotState, WizardState, clear_wizard_state, set_wizard_state};
use teloxide::payloads::AnswerCallbackQuerySetters;
use teloxide::prelude::{Bot, CallbackQuery, ChatId, Requester};
use teloxide::types::{CallbackQueryId, MessageId};

pub async fn ack_callback(
    bot: &Bot,
    callback_id: CallbackQueryId,
    text: Option<&str>,
    show_alert: bool,
) -> Result<(), anyhow::Error> {
    let mut request = bot.answer_callback_query(callback_id);
    if let Some(text) = text {
        request = request.text(text);
    }
    if show_alert {
        request = request.show_alert(true);
    }
    request.await?;
    Ok(())
}

pub async fn admin_callback_target(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
) -> Result<Option<(i64, ChatId, MessageId)>, anyhow::Error> {
    let Some(admin_id) = require_admin_callback(bot, q, state).await? else {
        return Ok(None);
    };
    let Some((chat_id, message_id)) = callback_message_target(q) else {
        ack_callback(bot, q.id.clone(), Some("Сообщение недоступно"), true).await?;
        return Ok(None);
    };
    Ok(Some((admin_id, chat_id, message_id)))
}

pub async fn replace_wizard_state(
    state: &BotState,
    user_id: i64,
    wizard_state: WizardState,
) -> Result<(), anyhow::Error> {
    clear_wizard_state(state, user_id).await?;
    set_wizard_state(state, user_id, wizard_state).await
}

pub async fn start_wizard_from_callback(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
    ack_text: &str,
    prompt_text: String,
) -> HandlerResult {
    let Some((admin_id, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
        return Ok(());
    };
    let wizard_state = match action {
        CallbackAction::PromptUserLookup { page } => {
            WizardState::AdminFindUserAwaitingTarget { page }
        }
        CallbackAction::PromptUserLimit {
            tg_user_id,
            page,
            field,
        } => WizardState::AdminSetUserLimitAwaitingValue {
            tg_user_id,
            page,
            field,
        },
        CallbackAction::PromptTokenLookup { page } => {
            WizardState::AdminFindTokenAwaitingCode { page }
        }
        CallbackAction::PromptTokenCreate { auto_approve } => {
            WizardState::AdminTokenCreateAwaitingParams { auto_approve }
        }
        CallbackAction::PromptCreateUser => WizardState::AdminCreateAwaitingTarget,
        CallbackAction::PromptDeleteUser => WizardState::AdminDeleteAwaitingTarget,
        _ => {
            return Err(anyhow::anyhow!(
                "start_wizard_from_callback вызван с неподдерживаемым действием"
            )
            .into());
        }
    };

    replace_wizard_state(state, admin_id, wizard_state).await?;
    ack_callback(bot, q.id.clone(), Some(ack_text), false).await?;
    bot.send_message(chat_id, prompt_text).await?;
    Ok(())
}
