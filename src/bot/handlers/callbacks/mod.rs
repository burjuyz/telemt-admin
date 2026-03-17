mod admin;
mod common;
mod user;

use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::shared::{
    HandlerResult, callback_message_target, send_admin_backend_error,
};
use crate::bot::handlers::state::BotState;
use teloxide::prelude::*;

pub fn handler()
-> teloxide::dispatching::UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    Update::filter_callback_query().endpoint(handle_callback)
}

async fn handle_callback(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    let is_admin = state.config.is_admin(q.from.id.0 as i64);
    let chat_id = callback_message_target(&q).map(|(chat_id, _)| chat_id);
    let callback_id = q.id.clone();
    let result = handle_callback_inner(bot.clone(), q, state).await;
    if let Err(error) = result {
        let error_text = error.to_string();
        if error_text.contains("message is not modified") {
            tracing::debug!(
                "Пропущено пустое обновление callback-сообщения: {}",
                error_text
            );
            return Ok(());
        }
        tracing::error!(error = %error, "Ошибка выполнения callback-действия");
        if is_admin {
            let _ = bot
                .answer_callback_query(callback_id)
                .text("Ошибка backend")
                .show_alert(true)
                .await;
            if let Some(chat_id) = chat_id {
                send_admin_backend_error(&bot, chat_id, "действие в админ-панели", error.as_ref())
                    .await;
            }
        }
    }
    Ok(())
}

async fn handle_callback_inner(bot: Bot, q: CallbackQuery, state: BotState) -> HandlerResult {
    let Some(data) = q.data.as_deref() else {
        return Ok(());
    };
    let Some(action) = CallbackAction::decode(data) else {
        common::ack_callback(
            &bot,
            q.id.clone(),
            Some("Устаревшая или некорректная кнопка"),
            true,
        )
        .await?;
        return Ok(());
    };

    if matches!(action, CallbackAction::Noop) {
        common::ack_callback(&bot, q.id.clone(), None, false).await?;
        return Ok(());
    }

    if user::handle_user_action(&bot, &q, &state, action.clone()).await? {
        return Ok(());
    }
    if admin::handle_admin_action(&bot, &q, &state, action).await? {
        return Ok(());
    }

    Ok(())
}
