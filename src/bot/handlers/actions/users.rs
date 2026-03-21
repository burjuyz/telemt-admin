use super::access::approve_user_direct_and_build_link;
use crate::bot::handlers::format::user_display_name;
use crate::bot::keyboards::user_lookup_candidates_keyboard;
use crate::bot::handlers::screens::{show_delete_user_confirm, show_user_card};
use crate::bot::handlers::state::{BotState, telemt_username};
use crate::db::RegistrationRequest;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::{Bot, ChatId, Requester};

const USER_PARTIAL_SEARCH_LIMIT: i64 = 15;

enum LookupInputKind<'a> {
    UserId(i64),
    /// После `@`, для точного совпадения username и при необходимости частичного поиска.
    Username(&'a str),
    /// Произвольная подстрока (имя, фрагмент @username, логин).
    PartialQuery(&'a str),
}

fn classify_lookup_input(arg: &str) -> Option<LookupInputKind<'_>> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(user_id) = trimmed.parse::<i64>() {
        return Some(LookupInputKind::UserId(user_id));
    }

    if let Some(username) = trimmed.strip_prefix('@') {
        let username = username.trim();
        if username.is_empty() {
            return None;
        }
        return Some(LookupInputKind::Username(username));
    }

    Some(LookupInputKind::PartialQuery(trimmed))
}

/// Разрешение активного пользователя по вводу админа: ID, @username или частичное совпадение.
async fn resolve_active_user_candidates(
    state: &BotState,
    arg: &str,
) -> Result<Vec<RegistrationRequest>, anyhow::Error> {
    let Some(kind) = classify_lookup_input(arg) else {
        return Ok(Vec::new());
    };

    match kind {
        LookupInputKind::UserId(id) => {
            let user = state.db.get_active_user_by_tg_user(id).await?;
            Ok(user.into_iter().collect())
        }
        LookupInputKind::Username(username) => {
            if let Some(id) = state.db.find_tg_user_id_by_username(username).await? {
                let user = state.db.get_active_user_by_tg_user(id).await?;
                return Ok(user.into_iter().collect());
            }
            state
                .db
                .search_active_users_by_partial(username, USER_PARTIAL_SEARCH_LIMIT)
                .await
        }
        LookupInputKind::PartialQuery(query) => {
            state
                .db
                .search_active_users_by_partial(query, USER_PARTIAL_SEARCH_LIMIT)
                .await
        }
    }
}

pub async fn create_user_from_input(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    arg: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let candidates = resolve_active_user_candidates(state, arg).await?;

    if candidates.is_empty() {
        bot.send_message(
            chat_id,
            "Пользователь не найден в базе.\n\
             Он должен хотя бы раз отправить боту /start.\n\
             Укажите Telegram ID, @username или часть имени/ника.",
        )
        .await?;
        return Ok(false);
    }

    if candidates.len() > 1 {
        bot.send_message(
            chat_id,
            format!(
                "Найдено несколько пользователей ({}). Уточните запрос: точный @username или Telegram ID.",
                candidates.len()
            ),
        )
        .await?;
        return Ok(false);
    }

    let tg_user_id = candidates[0].tg_user_id;
    tracing::info!(tg_user_id = tg_user_id, "Admin create user");
    let telemt_user = telemt_username(tg_user_id);
    let link = approve_user_direct_and_build_link(state, tg_user_id, None, None).await?;

    bot.send_message(
        chat_id,
        format!("Пользователь {} создан.\nСсылка:\n{}", telemt_user, link),
    )
    .await?;
    Ok(true)
}

pub async fn prompt_delete_confirmation(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    arg: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    match arg.trim().parse::<i64>() {
        Ok(tg_user_id) => {
            if state
                .db
                .get_active_user_by_tg_user(tg_user_id)
                .await?
                .is_none()
            {
                bot.send_message(
                    chat_id,
                    format!(
                        "Активный пользователь с Telegram ID {} не найден.",
                        tg_user_id
                    ),
                )
                .await?;
                return Ok(false);
            }
            show_delete_user_confirm(bot, chat_id, tg_user_id).await?;
            Ok(true)
        }
        Err(_) => {
            bot.send_message(chat_id, "Нужен корректный Telegram ID пользователя.")
                .await?;
            Ok(false)
        }
    }
}

pub async fn open_user_from_lookup_input(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    arg: &str,
    page: i64,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let Some(kind) = classify_lookup_input(arg) else {
        bot.send_message(
            chat_id,
            "Укажите Telegram ID, @username или часть имени/ника одним сообщением.",
        )
        .await?;
        return Ok(false);
    };

    // Сохраняем прежнее поведение: числовой ID без поиска в частичной таблице (уже в resolve).
    let candidates = resolve_active_user_candidates(state, arg).await?;

    if candidates.is_empty() {
        let hint = match kind {
            LookupInputKind::Username(u) => format!("Пользователь @{} не найден среди активных.", u),
            _ => "Активный пользователь не найден.".to_string(),
        };
        bot.send_message(chat_id, hint).await?;
        return Ok(false);
    }

    if candidates.len() > 1 {
        let pairs: Vec<(i64, String)> = candidates
            .iter()
            .map(|u| {
                let name = user_display_name(u);
                let short = if name.chars().count() > 48 {
                    format!("{}...", name.chars().take(45).collect::<String>())
                } else {
                    name
                };
                (u.tg_user_id, format!("{} · id {}", short, u.tg_user_id))
            })
            .collect();
        let keyboard = user_lookup_candidates_keyboard(&pairs, page);
        bot.send_message(
            chat_id,
            format!(
                "Найдено {} пользователей. Выберите:",
                candidates.len()
            ),
        )
        .reply_markup(keyboard)
        .await?;
        return Ok(true);
    }

    let user = &candidates[0];
    show_user_card(bot, chat_id, None, user, page).await?;
    Ok(true)
}

pub async fn has_active_users(
    state: &BotState,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    Ok(state.db.count_active_users().await? > 0)
}
