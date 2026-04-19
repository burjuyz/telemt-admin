//! Рассылка сообщений всем пользователям со статусом approved.

use crate::bot::handlers::shared::HandlerResult;
use crate::bot::handlers::state::{BotState, clear_wizard_state};
use std::time::Duration;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::{Bot, ChatId, Message, Requester};
use teloxide::types::ParseMode;

/// Проверяет, является ли текст командой отмены рассылки.
fn is_cancel_command(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    matches!(lower.as_str(), "отмена" | "/cancel" | "/отмена" | "cancel")
}

pub async fn broadcast_to_approved_users(
    bot: &Bot,
    msg: &Message,
    state: &BotState,
    admin_tg_user_id: i64,
    text: &str,
) -> HandlerResult {
    let trimmed = text.trim();

    // Отмена рассылки по команде или пустому тексту
    if trimmed.is_empty() || is_cancel_command(trimmed) {
        clear_wizard_state(state, admin_tg_user_id).await?;
        let cancel_message = state.config.bot_messages.broadcast_cancelled_or_default();

        // Гарантируем, что сообщение не пустое
        let message = if cancel_message.trim().is_empty() {
            "Рассылка отменена."
        } else {
            cancel_message
        };

        bot.send_message(msg.chat.id, message).await?;
        return Ok(());
    }

    let ids = state.db.list_approved_tg_user_ids().await?;
    let mut ok: u64 = 0;
    let mut failed: u64 = 0;

    for tg_user_id in ids {
        match bot
            .send_message(ChatId(tg_user_id), trimmed)
            .parse_mode(ParseMode::Html)
            .await
        {
            Ok(_) => ok += 1,
            Err(error) => {
                failed += 1;
                tracing::warn!(
                    target = tg_user_id,
                    error = %error,
                    "Не удалось доставить сообщение рассылки"
                );
            }
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }

    clear_wizard_state(state, admin_tg_user_id).await?;

    bot.send_message(
        msg.chat.id,
        state
            .config
            .bot_messages
            .broadcast_summary_text(ok, failed, ok + failed),
    )
    .await?;
    Ok(())
}
