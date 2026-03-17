use crate::bot::handlers::format::format_timestamp;
use crate::bot::handlers::shared::HandlerResult;
use crate::bot::handlers::state::{BotState, clear_wizard_state, telemt_username};
use crate::db::{
    ConsumedInviteToken, RegisterResult, RegistrationRequest, TokenConsumeError, TokenMode,
};
use crate::link::generate_user_secret;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::{Bot, ChatId, Message, Requester};

async fn notify_auto_approve(
    bot: &Bot,
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
    token: &ConsumedInviteToken,
) {
    let mode_label = match token.mode {
        TokenMode::AutoApprove => "auto",
        TokenMode::Manual => "manual",
    };
    let text = format!(
        "✅ Автоподключение по токену\n\
         User ID: {}\n\
         Username: @{}\n\
         Имя: {}\n\
         Token: {}\n\
         Token ID: {}\n\
         Mode: {}\n\
         Expires: {}\n\
         Usage: {}/{}\n\
         Created by: {}",
        tg_user_id,
        tg_username.unwrap_or("—"),
        tg_display_name.unwrap_or("—"),
        token.token,
        token.id,
        mode_label,
        format_timestamp(token.expires_at),
        token.usage_count,
        token
            .max_usage
            .map(|value| value.to_string())
            .unwrap_or_else(|| "∞".to_string()),
        token
            .created_by
            .map(|value| value.to_string())
            .unwrap_or_else(|| "—".to_string())
    );

    for admin_id in &state.config.admin_ids {
        if let Err(error) = bot.send_message(ChatId(*admin_id), text.clone()).await {
            tracing::warn!(
                admin_id = *admin_id,
                error = %error,
                "Не удалось отправить аудит автоподключения"
            );
        }
    }
}

async fn notify_admins(bot: &Bot, state: &BotState, req: &RegistrationRequest) -> HandlerResult {
    let text = format!(
        "📋 Новая заявка #{}:\n\
         User ID: {}\n\
         Username: @{}\n\
         Имя: {}\n\
         Время: {}",
        req.id,
        req.tg_user_id,
        req.tg_username.as_deref().unwrap_or("—"),
        req.tg_display_name.as_deref().unwrap_or("—"),
        format_timestamp(req.created_at),
    );

    let kb = crate::bot::keyboards::approve_reject_buttons(req.id);
    for admin_id in &state.config.admin_ids {
        if let Err(error) = bot
            .send_message(ChatId(*admin_id), text.clone())
            .reply_markup(kb.clone())
            .await
        {
            tracing::warn!(
                admin_id = *admin_id,
                error = %error,
                "Не удалось отправить уведомление админу"
            );
        }
    }
    Ok(())
}

pub async fn approve_request_and_build_link(
    state: &BotState,
    request_id: i64,
) -> Result<Option<(RegistrationRequest, String)>, anyhow::Error> {
    let request = match state.db.get_pending_by_id(request_id).await? {
        Some(request) => request,
        None => return Ok(None),
    };

    let telemt_user = telemt_username(request.tg_user_id);
    let user_secret = generate_user_secret();
    let provisioned = state
        .telemt_backend
        .provision_user(&telemt_user, &user_secret)
        .await?;
    if state
        .db
        .approve(request_id, &telemt_user, &provisioned.secret)
        .await?
        .is_none()
    {
        return Ok(None);
    }
    state
        .db
        .mark_sync_state(
            request.tg_user_id,
            provisioned.mode.as_str(),
            provisioned.revision.as_deref(),
            None,
        )
        .await?;
    let proxy_link = if let Some(link) = provisioned.link {
        link
    } else {
        state
            .telemt_backend
            .build_user_link(&telemt_user, Some(&provisioned.secret))
            .await?
    };
    Ok(Some((request, proxy_link)))
}

pub async fn approve_user_direct_and_build_link(
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
) -> Result<String, anyhow::Error> {
    let telemt_user = telemt_username(tg_user_id);
    let secret = generate_user_secret();
    let provisioned = state
        .telemt_backend
        .provision_user(&telemt_user, &secret)
        .await?;
    state
        .db
        .set_approved(
            tg_user_id,
            tg_username,
            tg_display_name,
            &telemt_user,
            &provisioned.secret,
        )
        .await?;
    state
        .db
        .mark_sync_state(
            tg_user_id,
            provisioned.mode.as_str(),
            provisioned.revision.as_deref(),
            None,
        )
        .await?;
    if let Some(link) = provisioned.link {
        Ok(link)
    } else {
        state
            .telemt_backend
            .build_user_link(&telemt_user, Some(&provisioned.secret))
            .await
    }
}

pub async fn process_invite_token(
    bot: &Bot,
    msg: &Message,
    state: &BotState,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
    token: &str,
) -> HandlerResult {
    let consumed = match state.db.consume_invite_token(token).await {
        Ok(token_payload) => token_payload,
        Err(TokenConsumeError::NotFound) => {
            bot.send_message(
                msg.chat.id,
                "Токен не найден. Проверьте код и попробуйте снова.",
            )
            .await?;
            return Ok(());
        }
        Err(TokenConsumeError::Revoked) => {
            bot.send_message(msg.chat.id, "Этот токен отозван администратором.")
                .await?;
            return Ok(());
        }
        Err(TokenConsumeError::Expired) => {
            bot.send_message(msg.chat.id, "Срок действия токена истёк.")
                .await?;
            return Ok(());
        }
        Err(TokenConsumeError::UsageLimitReached) => {
            bot.send_message(msg.chat.id, "Лимит использований токена исчерпан.")
                .await?;
            return Ok(());
        }
        Err(TokenConsumeError::Internal(error)) => return Err(error.into()),
    };

    tracing::info!(
        tg_user_id = tg_user_id,
        token = %consumed.token,
        token_id = consumed.id,
        mode = ?consumed.mode,
        usage_count = consumed.usage_count,
        max_usage = ?consumed.max_usage,
        expires_at = consumed.expires_at,
        "Токен успешно применён"
    );

    match consumed.mode {
        TokenMode::Manual => {
            let result = state
                .db
                .register_or_get(tg_user_id, tg_username, tg_display_name)
                .await?;
            match result {
                RegisterResult::Approved(secret) => {
                    let link = state
                        .telemt_backend
                        .build_user_link(&telemt_username(tg_user_id), Some(&secret))
                        .await?;
                    bot.send_message(msg.chat.id, format!("Ваша ссылка на прокси:\n\n{}", link))
                        .await?;
                    clear_wizard_state(state, tg_user_id).await?;
                }
                RegisterResult::Rejected => {
                    bot.send_message(
                        msg.chat.id,
                        "Ваша заявка на регистрацию отклонена администратором.",
                    )
                    .await?;
                    clear_wizard_state(state, tg_user_id).await?;
                }
                RegisterResult::AlreadyPending => {
                    bot.send_message(
                        msg.chat.id,
                        "Ваша заявка уже на рассмотрении. Ожидайте подтверждения администратора.",
                    )
                    .await?;
                    clear_wizard_state(state, tg_user_id).await?;
                }
                RegisterResult::NewPending(ref req) => {
                    bot.send_message(msg.chat.id, "Заявка отправлена. Ожидайте подтверждения.")
                        .await?;
                    notify_admins(bot, state, req).await?;
                    clear_wizard_state(state, tg_user_id).await?;
                }
            }
        }
        TokenMode::AutoApprove => {
            let link =
                approve_user_direct_and_build_link(state, tg_user_id, tg_username, tg_display_name)
                    .await?;
            bot.send_message(
                msg.chat.id,
                format!("Доступ одобрен! Ваша ссылка для подключения:\n\n{}", link),
            )
            .await?;
            notify_auto_approve(
                bot,
                state,
                tg_user_id,
                tg_username,
                tg_display_name,
                &consumed,
            )
            .await;
            clear_wizard_state(state, tg_user_id).await?;
        }
    }

    Ok(())
}

pub async fn send_user_link(
    bot: &Bot,
    chat_id: ChatId,
    tg_user_id: i64,
    tg_username: Option<&str>,
    tg_display_name: Option<&str>,
    state: &BotState,
) -> HandlerResult {
    let maybe = state.db.get_approved(tg_user_id).await?;
    match maybe {
        Some((telemt_user, secret)) => {
            let link = state
                .telemt_backend
                .build_user_link(&telemt_user, Some(&secret))
                .await?;
            bot.send_message(chat_id, format!("Ваша ссылка на прокси:\n\n{}", link))
                .await?;
        }
        None => {
            if state.config.is_admin(tg_user_id) {
                tracing::info!(
                    tg_user_id = tg_user_id,
                    "Администратор запросил ссылку без существующей учётной записи, создаём доступ автоматически"
                );
                let link = approve_user_direct_and_build_link(
                    state,
                    tg_user_id,
                    tg_username,
                    tg_display_name,
                )
                .await?;
                bot.send_message(chat_id, format!("Ваша ссылка на прокси:\n\n{}", link))
                    .await?;
            } else {
                bot.send_message(
                    chat_id,
                    "У вас нет доступа к прокси. Отправьте /start для регистрации.",
                )
                .await?;
            }
        }
    }
    Ok(())
}

pub async fn perform_hard_ban(state: &BotState, tg_user_id: i64) -> Result<String, anyhow::Error> {
    let telemt_user = telemt_username(tg_user_id);
    let removed_from_cfg = state.telemt_backend.delete_user(&telemt_user).await?;
    let removed_from_db = state.db.deactivate_user(tg_user_id).await?;
    let sync_error = if removed_from_cfg {
        None
    } else {
        Some("user_not_found_in_backend")
    };
    state
        .db
        .mark_sync_state(
            tg_user_id,
            state.telemt_backend.mode().as_str(),
            None,
            sync_error,
        )
        .await?;

    if removed_from_cfg || removed_from_db {
        Ok(format!("Пользователь {} удалён", telemt_user))
    } else {
        Ok(format!("Пользователь {} не найден", telemt_user))
    }
}
