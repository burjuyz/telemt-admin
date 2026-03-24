use super::access::approve_user_direct_and_build_link;
use crate::bot::handlers::callback_data::UserLimitField;
use crate::bot::handlers::format::user_display_name;
use crate::bot::keyboards::user_lookup_candidates_keyboard;
use crate::bot::handlers::shared::build_bot_start_link;
use crate::bot::handlers::screens::{show_delete_user_confirm, show_user_card};
use crate::bot::handlers::state::{BotState, telemt_username};
use crate::db::RegistrationRequest;
use chrono::{Duration, NaiveDate, Utc};
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
    show_user_card(bot, chat_id, None, user, page, state).await?;
    Ok(true)
}

pub async fn has_active_users(
    state: &BotState,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    Ok(state.db.count_active_users().await? > 0)
}

fn parse_positive_usize(value: &str, label: &str) -> Result<usize, anyhow::Error> {
    let parsed = value
        .trim()
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("{label} должен быть положительным целым числом"))?;
    if parsed == 0 {
        return Err(anyhow::anyhow!("{label} должен быть больше нуля"));
    }
    Ok(parsed)
}

fn parse_bytes_input(value: &str) -> Result<u64, anyhow::Error> {
    let trimmed = value.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("Нужно указать объём трафика"));
    }

    let split_idx = trimmed
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (number, suffix) = trimmed.split_at(split_idx);
    let base = number
        .parse::<u64>()
        .map_err(|_| anyhow::anyhow!("Некорректное число для квоты трафика"))?;
    if base == 0 {
        return Err(anyhow::anyhow!("Квота трафика должна быть больше нуля"));
    }

    let multiplier = match suffix.trim() {
        "" | "b" => 1,
        "k" | "kb" => 1024,
        "m" | "mb" => 1024_u64.pow(2),
        "g" | "gb" => 1024_u64.pow(3),
        "t" | "tb" => 1024_u64.pow(4),
        _ => {
            return Err(anyhow::anyhow!(
                "Неизвестная единица. Поддерживаются B, KB, MB, GB, TB"
            ));
        }
    };
    base.checked_mul(multiplier)
        .ok_or_else(|| anyhow::anyhow!("Квота слишком большая"))
}

fn parse_expiration_input(value: &str) -> Result<String, anyhow::Error> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("Нужно указать дату истечения"));
    }

    if let Ok(date_time) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Ok(date_time.to_rfc3339());
    }

    if let Some(days) = trimmed.strip_prefix('+') {
        let days = days.trim_end_matches('d').trim();
        let days = parse_positive_usize(days, "Количество дней")? as i64;
        return Ok((Utc::now() + Duration::days(days)).to_rfc3339());
    }

    let date = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("Используйте RFC3339 или дату в формате YYYY-MM-DD"))?;
    let date_time = date
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| anyhow::anyhow!("Не удалось собрать дату истечения"))?;
    Ok(chrono::DateTime::<Utc>::from_naive_utc_and_offset(date_time, Utc).to_rfc3339())
}

fn user_limit_success_label(field: UserLimitField, value: &str) -> String {
    match field {
        UserLimitField::MaxTcpConns => format!("Лимит TCP-соединений обновлён: {value}"),
        UserLimitField::DataQuotaBytes => format!("Квота трафика обновлена: {value}"),
        UserLimitField::MaxUniqueIps => format!("Лимит уникальных IP обновлён: {value}"),
        UserLimitField::Expiration => format!("Дата истечения обновлена: {value}"),
    }
}

pub fn user_limit_input_help(field: UserLimitField) -> String {
    match field {
        UserLimitField::MaxTcpConns => {
            "Отправьте положительное целое число следующим сообщением.\n\nПример: 5".to_string()
        }
        UserLimitField::DataQuotaBytes => "Отправьте объём трафика числом или с суффиксом KB/MB/GB/TB.\n\nПримеры: 1073741824, 10GB".to_string(),
        UserLimitField::MaxUniqueIps => {
            "Отправьте положительное целое число следующим сообщением.\n\nПример: 3".to_string()
        }
        UserLimitField::Expiration => "Отправьте RFC3339-время, дату `YYYY-MM-DD` или относительное значение вида `+30d`.\n\nПримеры: 2026-04-30, 2026-04-30T18:00:00+00:00, +30d".to_string(),
    }
}

pub async fn apply_user_limit_from_input(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    tg_user_id: i64,
    field: UserLimitField,
    arg: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
        bot.send_message(chat_id, "Активный пользователь не найден.")
            .await?;
        return Ok(false);
    };
    let Some(telemt_user) = user.telemt_username.as_deref() else {
        bot.send_message(chat_id, "У пользователя не найден telemt username.")
            .await?;
        return Ok(false);
    };

    let (patch, success_value) = match field {
        UserLimitField::MaxTcpConns => {
            let value = parse_positive_usize(arg, "Лимит TCP-соединений")?;
            (
                crate::telemt_backend::TelemtUserPatch {
                    max_tcp_conns: Some(value),
                    ..Default::default()
                },
                value.to_string(),
            )
        }
        UserLimitField::DataQuotaBytes => {
            let value = parse_bytes_input(arg)?;
            (
                crate::telemt_backend::TelemtUserPatch {
                    data_quota_bytes: Some(value),
                    ..Default::default()
                },
                value.to_string(),
            )
        }
        UserLimitField::MaxUniqueIps => {
            let value = parse_positive_usize(arg, "Лимит уникальных IP")?;
            (
                crate::telemt_backend::TelemtUserPatch {
                    max_unique_ips: Some(value),
                    ..Default::default()
                },
                value.to_string(),
            )
        }
        UserLimitField::Expiration => {
            let value = parse_expiration_input(arg)?;
            (
                crate::telemt_backend::TelemtUserPatch {
                    expiration_rfc3339: Some(value.clone()),
                    ..Default::default()
                },
                value,
            )
        }
    };

    state.telemt_backend.patch_user(telemt_user, &patch).await?;
    bot.send_message(chat_id, user_limit_success_label(field, &success_value))
        .await?;
    Ok(true)
}

pub async fn send_user_start_link(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    tg_user_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some(bot_username) = state.bot_username.as_deref() else {
        bot.send_message(
            chat_id,
            "Не удалось определить username бота. Укажите `bot_username` в конфиге \
             или `TELEMT_ADMIN__BOT_USERNAME`, если getMe к Telegram недоступен.",
        )
        .await?;
        return Ok(());
    };
    let link = build_bot_start_link(bot_username, &format!("admin:user:{tg_user_id}"));
    bot.send_message(
        chat_id,
        format!(
            "Deep link для карточки пользователя:\n{}\n\nОткроется карточка пользователя после /start.",
            link
        ),
    )
    .await?;
    Ok(())
}

pub async fn send_token_start_link(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    token_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some(bot_username) = state.bot_username.as_deref() else {
        bot.send_message(
            chat_id,
            "Не удалось определить username бота. Укажите `bot_username` в конфиге \
             или `TELEMT_ADMIN__BOT_USERNAME`, если getMe к Telegram недоступен.",
        )
        .await?;
        return Ok(());
    };
    let link = build_bot_start_link(bot_username, &format!("admin:token:{token_id}"));
    bot.send_message(
        chat_id,
        format!(
            "Deep link для карточки токена:\n{}\n\nОткроется карточка токена после /start.",
            link
        ),
    )
    .await?;
    Ok(())
}
