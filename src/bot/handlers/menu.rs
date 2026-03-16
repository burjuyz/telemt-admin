use super::commands::{
    create_user_from_input, handle_token_create_from_text, prompt_delete_confirmation,
};
use super::shared::{process_invite_token, HandlerResult};
use super::state::{
    clear_wizard_state, sender_display_name, sender_user_id, wizard_state, BotState, WizardState,
};
use teloxide::prelude::*;

pub async fn handle_menu_buttons(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let Some(user_id) = sender_user_id(&msg) else {
        return Ok(());
    };

    if text.starts_with('/') {
        bot.send_message(msg.chat.id, "Неизвестная команда. Используйте /help.")
            .await?;
        return Ok(());
    }

    match wizard_state(&state, user_id).await? {
        Some(WizardState::AwaitingInviteToken) => {
            let username = msg.from.as_ref().and_then(|u| u.username.clone());
            let display_name = sender_display_name(&msg);
            process_invite_token(
                &bot,
                &msg,
                &state,
                user_id,
                username.as_deref(),
                display_name.as_deref(),
                text.trim(),
            )
            .await?;
        }
        Some(WizardState::AdminCreateAwaitingTarget) => {
            let created = create_user_from_input(&bot, msg.chat.id, &state, text.trim()).await?;
            if created {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        Some(WizardState::AdminDeleteAwaitingTarget) => {
            let prompted = prompt_delete_confirmation(&bot, msg.chat.id, text.trim()).await?;
            if prompted {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        Some(WizardState::AdminTokenCreateAwaitingParams { auto_approve }) => {
            let created = handle_token_create_from_text(
                &bot,
                msg.chat.id,
                &state,
                auto_approve,
                text.trim(),
                Some(user_id),
            )
            .await?;
            if created {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        Some(WizardState::AdminTokenRevokeAwaitingToken) => {
            let token = text.trim();
            if token.is_empty() {
                bot.send_message(msg.chat.id, "Отправьте код токена одним сообщением.")
                    .await?;
            } else {
                let revoked = state.db.revoke_invite_token(token).await?;
                if revoked {
                    clear_wizard_state(&state, user_id).await?;
                    bot.send_message(msg.chat.id, format!("Токен {} отозван.", token))
                        .await?;
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Токен не найден или уже отозван. Можно отправить другой код.",
                    )
                    .await?;
                }
            }
        }
        None => {
            bot.send_message(
                msg.chat.id,
                "Не понял запрос. Используйте /help или начните нужный сценарий через slash-команду.",
            )
            .await?;
        }
    }
    Ok(())
}
