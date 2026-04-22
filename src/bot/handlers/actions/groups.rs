//! Массовые операции по группам пользователей.

use crate::bot::handlers::actions::access::perform_hard_ban;
use crate::bot::handlers::state::BotState;
use crate::db::UserGroup;
use crate::telemt_backend::TelemtUserPatch;
use chrono::{TimeZone, Utc};

pub async fn deactivate_all_members(
    state: &BotState,
    group: &UserGroup,
) -> Result<(usize, usize), anyhow::Error> {
    let ids = state.db.list_group_member_tg_ids(group.id).await?;
    let mut ok = 0usize;
    let mut err = 0usize;
    for tg_user_id in ids {
        match perform_hard_ban(state, tg_user_id).await {
            Ok(_) => ok += 1,
            Err(e) => {
                err += 1;
                tracing::warn!(
                    group_id = group.id,
                    tg_user_id,
                    error = %e,
                    "Не удалось отключить пользователя группы"
                );
            }
        }
    }
    let _ = state.db.delete_user_group(group.id).await?;
    Ok((ok, err))
}

pub async fn apply_group_expiry_to_members(
    state: &BotState,
    group: &UserGroup,
) -> Result<(usize, usize), anyhow::Error> {
    let Some(exp_unix) = group.expires_at else {
        return Err(anyhow::anyhow!(
            "У группы не задан общий срок действия (expires_at). Задайте срок в карточке группы."
        ));
    };
    let dt = Utc
        .timestamp_opt(exp_unix, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("Некорректное время expires_at у группы"))?;
    let rfc = dt.to_rfc3339();

    let ids = state.db.list_group_member_tg_ids(group.id).await?;
    let mut ok = 0usize;
    let mut err = 0usize;

    for tg_user_id in ids {
        let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
            err += 1;
            continue;
        };
        let Some(telemt_username) = user.telemt_username.as_deref() else {
            err += 1;
            continue;
        };
        let patch = TelemtUserPatch {
            expiration_rfc3339: Some(rfc.clone()),
            ..Default::default()
        };
        match state
            .telemt_backend
            .patch_user(telemt_username, &patch)
            .await
        {
            Ok(_) => ok += 1,
            Err(e) => {
                err += 1;
                tracing::warn!(
                    telemt_username,
                    error = %e,
                    "PATCH expiration для участника группы"
                );
            }
        }
    }

    Ok((ok, err))
}

pub async fn apply_group_limits_to_members(
    state: &BotState,
    group: &UserGroup,
) -> Result<(usize, usize), anyhow::Error> {
    if group.default_expiration_days.is_none()
        && group.default_max_unique_ips.is_none()
        && group.default_data_quota_bytes.is_none()
    {
        return Err(anyhow::anyhow!(
            "У группы не заданы лимиты. Задайте лимиты в карточке группы."
        ));
    }

    let expiration_rfc3339 = if let Some(days) = group.default_expiration_days {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let expiry_ts = now + (days as i64 * 24 * 60 * 60);
        Some(
            chrono::DateTime::from_timestamp(expiry_ts, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "2025-12-31T23:59:59Z".to_string()),
        )
    } else {
        None
    };

    let ids = state.db.list_group_member_tg_ids(group.id).await?;
    let mut ok = 0usize;
    let mut err = 0usize;

    for tg_user_id in ids {
        let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
            err += 1;
            continue;
        };
        let Some(telemt_username) = user.telemt_username.as_deref() else {
            err += 1;
            continue;
        };
        let patch = TelemtUserPatch {
            expiration_rfc3339: expiration_rfc3339.clone(),
            max_unique_ips: group.default_max_unique_ips.map(|v| v as usize),
            data_quota_bytes: group.default_data_quota_bytes.map(|v| v as u64),
            ..Default::default()
        };
        match state
            .telemt_backend
            .patch_user(telemt_username, &patch)
            .await
        {
            Ok(_) => ok += 1,
            Err(e) => {
                err += 1;
                tracing::warn!(
                    telemt_username,
                    error = %e,
                    "PATCH limits для участника группы"
                );
            }
        }
    }

    Ok((ok, err))
}
