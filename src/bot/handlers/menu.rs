use super::actions::{
    apply_user_limit_from_input, handle_token_create_from_text, open_token_from_lookup_input,
    open_user_from_lookup_input, process_invite_token, prompt_delete_confirmation,
};
use super::shared::{HandlerResult, send_admin_backend_error};
use super::state::{
    BotState, WizardState, clear_wizard_state, is_admin_message, sender_display_name,
    sender_user_id, wizard_state,
};
use teloxide::prelude::*;

pub async fn handle_menu_buttons(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let is_admin = is_admin_message(&msg, &state);
    let chat_id = msg.chat.id;
    let result = handle_menu_buttons_inner(bot.clone(), msg, state).await;
    if let Err(error) = result {
        tracing::error!(error = %error, "Ошибка выполнения текстового сценария");
        if is_admin {
            send_admin_backend_error(&bot, chat_id, "текстовый шаг сценария", error.as_ref()).await;
        }
    }
    Ok(())
}

async fn handle_menu_buttons_inner(bot: Bot, msg: Message, state: BotState) -> HandlerResult {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let Some(user_id) = sender_user_id(&msg) else {
        return Ok(());
    };

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
        Some(WizardState::AdminDeleteAwaitingTarget) => {
            let prompted =
                prompt_delete_confirmation(&bot, msg.chat.id, &state, text.trim()).await?;
            if prompted {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        Some(WizardState::AdminFindUserAwaitingTarget { page }) => {
            let opened =
                open_user_from_lookup_input(&bot, msg.chat.id, &state, text.trim(), page).await?;
            if opened {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        Some(WizardState::AdminSetUserLimitAwaitingValue {
            tg_user_id,
            page: _,
            field,
        }) => {
            let updated =
                apply_user_limit_from_input(&bot, msg.chat.id, &state, tg_user_id, field, text.trim())
                    .await?;
            if updated {
                clear_wizard_state(&state, user_id).await?;
            }
        }
        Some(WizardState::AdminFindTokenAwaitingCode { page }) => {
            let opened =
                open_token_from_lookup_input(&bot, msg.chat.id, &state, text.trim(), page).await?;
            if opened {
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
        None => {
            bot.send_message(
                msg.chat.id,
                "Не понял запрос. Используйте /help или начните нужный сценарий через slash-команду либо кнопку.",
            )
            .await?;
        }
    }
    Ok(())
}
