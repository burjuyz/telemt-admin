use super::super::common::{ack_callback, admin_callback_target};
use super::AdminActionResult;
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{admin_show_stats, show_admin_home};
use crate::bot::handlers::state::{BotState, clear_wizard_state};
use teloxide::prelude::{Bot, CallbackQuery};

pub async fn handle(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> AdminActionResult {
    match action {
        CallbackAction::ShowAdminHome => {
            let Some((admin_id, chat_id, message_id)) =
                admin_callback_target(bot, q, state).await?
            else {
                return Ok(true);
            };
            clear_wizard_state(state, admin_id).await?;
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_admin_home(bot, chat_id, Some(message_id)).await?;
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
        _ => Ok(false),
    }
}
