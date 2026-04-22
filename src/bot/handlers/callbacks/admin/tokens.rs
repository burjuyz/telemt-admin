use super::super::common::{ack_callback, admin_callback_target, replace_wizard_state, start_wizard_from_callback};
use super::AdminActionResult;
use crate::bot::handlers::actions::send_token_start_link;
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{
    admin_show_token_list_page, show_token_card, show_token_menu, show_token_revoke_confirm,
};
use crate::bot::handlers::shared::{callback_message_target, require_admin_callback};
use crate::bot::handlers::state::{clear_wizard_state, BotState, WizardState};
use crate::bot::keyboards;
use teloxide::payloads::EditMessageTextSetters;
use teloxide::prelude::{Bot, CallbackQuery, Requester};

pub async fn handle(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> AdminActionResult {
    match action {
        CallbackAction::ShowTokenMenu => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_token_menu(bot, chat_id, Some(message_id), state).await?;
            Ok(true)
        }
        CallbackAction::PromptTokenCreate { auto_approve } => {
            let Some((admin_id, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };

            let wizard_state = WizardState::AdminTokenAwaitingExpiration { auto_approve };
            replace_wizard_state(state, admin_id, wizard_state).await?;

            bot.edit_message_text(
                chat_id,
                message_id,
                "Выберите срок доступа:",
            )
            .reply_markup(keyboards::token_expiration_keyboard(auto_approve))
            .await?;

            Ok(true)
        }
        CallbackAction::SetTokenExpiration { days, auto_approve } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };

            let wizard = crate::bot::handlers::state::wizard_state(state, q.from.id.0 as i64).await?;
            if let Some(WizardState::AdminEditTokenLimits { token_id, page }) = wizard {
                let _updated = state.db.update_invite_token_limits(token_id, Some(days), None, None).await?;
                ack_callback(bot, q.id.clone(), Some(&format!("Срок: {} дн.", days)), false).await?;
                if let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? {
                    show_token_card(bot, chat_id, Some(message_id), state, &token, page).await?;
                }
                crate::bot::handlers::state::clear_wizard_state(state, q.from.id.0 as i64).await?;
                return Ok(true);
            }

            let new_state = WizardState::AdminTokenAwaitingMaxIps {
                auto_approve,
                expiration_days: Some(days),
            };
            replace_wizard_state(state, q.from.id.0 as i64, new_state).await?;

            let keyboard = keyboards::token_max_ips_keyboard(auto_approve, days);
            bot.edit_message_text(
                chat_id,
                message_id,
                format!(
                    "Срок доступа: {} дн.\n\n\
                     Теперь выберите лимит IP (Max Unique IPs):",
                    days
                ),
            )
            .reply_markup(keyboard)
            .await?;
            Ok(true)
        }
        CallbackAction::SetTokenMaxIps { count, auto_approve, expiration_days } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };

            let wizard = crate::bot::handlers::state::wizard_state(state, q.from.id.0 as i64).await?;
            if let Some(WizardState::AdminEditTokenLimits { token_id, page }) = wizard {
                let _updated = state.db.update_invite_token_limits(token_id, None, count, None).await?;
                let ip_text = count.map(|c| c.to_string()).unwrap_or_else(|| "без лимита".to_string());
                ack_callback(bot, q.id.clone(), Some(&format!("IP: {}", ip_text)), false).await?;
                if let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? {
                    show_token_card(bot, chat_id, Some(message_id), state, &token, page).await?;
                }
                crate::bot::handlers::state::clear_wizard_state(state, q.from.id.0 as i64).await?;
                return Ok(true);
            }

            let ip_text = count.map(|c| c.to_string()).unwrap_or_else(|| "без лимита".to_string());
            ack_callback(bot, q.id.clone(), Some(&format!("IP: {}", ip_text)), false).await?;

            let new_state = WizardState::AdminTokenAwaitingDataQuota {
                auto_approve,
                expiration_days: Some(expiration_days),
                max_unique_ips: count,
            };
            replace_wizard_state(state, q.from.id.0 as i64, new_state).await?;

            bot.edit_message_text(
                chat_id,
                message_id,
                format!(
                    "Срок доступа: {} дн.\nЛимит IP: {}\n\n\
                     Теперь выберите квоту трафика:",
                    expiration_days,
                    ip_text
                ),
            )
            .reply_markup(keyboards::token_data_quota_keyboard(auto_approve, expiration_days, count))
            .await?;
            Ok(true)
        }
        CallbackAction::SetTokenDataQuota { quota_gb, auto_approve, expiration_days, max_unique_ips } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };

            let wizard = crate::bot::handlers::state::wizard_state(state, q.from.id.0 as i64).await?;
            if let Some(WizardState::AdminEditTokenLimits { token_id, page }) = wizard {
                let data_quota_bytes = quota_gb.map(|gb| gb * 1024 * 1024 * 1024);
                let _updated = state.db.update_invite_token_limits(token_id, None, None, data_quota_bytes).await?;
                let quota_text = match quota_gb {
                    Some(0) => "безлимит".to_string(),
                    Some(gb) => format!("{} GB", gb),
                    None => "другое...".to_string(),
                };
                ack_callback(bot, q.id.clone(), Some(&format!("Квота: {}", quota_text)), false).await?;
                if let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? {
                    show_token_card(bot, chat_id, Some(message_id), state, &token, page).await?;
                }
                crate::bot::handlers::state::clear_wizard_state(state, q.from.id.0 as i64).await?;
                return Ok(true);
            }

            let quota_text = match quota_gb {
                Some(0) => "безлимит".to_string(),
                Some(gb) => format!("{} GB", gb),
                None => "другое...".to_string(),
            };

            let data_quota_bytes = quota_gb.map(|gb| gb * 1024 * 1024 * 1024);

            let new_state = WizardState::AdminTokenAwaitingGroup {
                auto_approve,
                expiration_days,
                max_unique_ips,
                data_quota_bytes,
            };
            replace_wizard_state(state, q.from.id.0 as i64, new_state).await?;

            let groups = state.db.list_user_groups().await?;

            bot.edit_message_text(
                chat_id,
                message_id,
                format!(
                    "Срок доступа: {} дн.\nЛимит IP: {}\nКвота: {}\n\n\
                     Выберите группу для токена:",
                    expiration_days,
                    max_unique_ips.map(|i| i.to_string()).unwrap_or_else(|| "—".to_string()),
                    quota_text
                ),
            )
            .reply_markup(keyboards::token_group_picker_keyboard(&groups))
            .await?;

            ack_callback(bot, q.id.clone(), Some(&format!("Квота: {}", quota_text)), false).await?;

            Ok(true)
        }
        CallbackAction::TokenAssignGroup { group_id } => {
            let Some((admin_id, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };

            let wizard = crate::bot::handlers::state::wizard_state(state, admin_id).await?;
            let (auto_approve, expiration_days, max_unique_ips, data_quota_bytes) = match wizard {
                Some(WizardState::AdminTokenAwaitingGroup {
                    auto_approve,
                    expiration_days,
                    max_unique_ips,
                    data_quota_bytes,
                }) => (auto_approve, expiration_days, max_unique_ips, data_quota_bytes),
                _ => {
                    ack_callback(bot, q.id.clone(), Some("Ошибка: неверное состояние"), true).await?;
                    return Ok(true);
                }
            };

            ack_callback(bot, q.id.clone(), None, false).await?;

            clear_wizard_state(state, admin_id).await?;

            let final_group_id = if group_id == 0 { None } else { Some(group_id) };

            let token = state
                .db
                .create_invite_token(
                    30,
                    auto_approve,
                    None,
                    Some(admin_id),
                    Some(expiration_days),
                    max_unique_ips,
                    data_quota_bytes,
                    final_group_id,
                )
                .await?;

            let group_name = if let Some(id) = final_group_id {
                state.db.get_user_group_by_id(id).await?
                    .map(|g| g.name)
                    .unwrap_or_else(|| format!("ID {}", id))
            } else {
                "без группы".to_string()
            };

            let link_line = state
                .bot_username
                .as_deref()
                .map(|bot_username| {
                    crate::bot::handlers::shared::build_bot_start_link(bot_username, &token.token)
                })
                .map(|link| format!("Ссылка: {}\n", link))
                .unwrap_or_else(|| "Ссылка: недоступна\n".to_string());

            let limits_text = {
                let mut parts = Vec::new();
                if let Some(days) = token.default_expiration_days {
                    parts.push(format!("доступ {} дн.", days));
                }
                if let Some(ips) = token.default_max_unique_ips {
                    parts.push(format!("IP: {}", ips));
                }
                if let Some(quota) = token.default_data_quota_bytes {
                    let gb = quota as f64 / 1_073_741_824.0;
                    parts.push(format!("{:.1} GB", gb));
                }
                parts.push(format!("группа: {}", group_name));
                if parts.is_empty() {
                    "по умолчанию".to_string()
                } else {
                    parts.join(", ")
                }
            };

            bot.edit_message_text(
                chat_id,
                message_id,
                format!(
                    "✅ Invite-токен создан:\n\
                     Код: <code>{}</code>\n\
                     {}\
                     Режим: {}\n\
                     Лимиты пользователя: {}",
                    token.token,
                    link_line,
                    if auto_approve { "AUTO" } else { "MANUAL" },
                    limits_text,
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
            Ok(true)
        }
        CallbackAction::ShowTokenList => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_token_list_page(bot, chat_id, state, 1, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ShowTokenListPage { page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_token_list_page(bot, chat_id, state, page, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::PromptTokenLookup { page } => {
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptTokenLookup { page },
                "Жду код токена",
                "Отправьте код токена следующим сообщением.\n\nСписок можно оставить открытым."
                    .to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::OpenTokenCard { token_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Токен уже недоступен"), true).await?;
                admin_show_token_list_page(bot, chat_id, state, page, Some(message_id)).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Открыта карточка токена"), false).await?;
            show_token_card(bot, chat_id, Some(message_id), state, &token, page).await?;
            Ok(true)
        }
        CallbackAction::PromptEditTokenGroup { token_id, page } => {
            let Some((admin_id, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };

            let wizard_state = WizardState::AdminEditTokenGroup { token_id, page };
            crate::bot::handlers::state::set_wizard_state(state, admin_id, wizard_state).await?;

            let groups = state.db.list_user_groups().await?;

            ack_callback(bot, q.id.clone(), None, false).await?;
            bot.edit_message_text(
                chat_id,
                message_id,
                "Выберите группу для токена:",
            )
            .reply_markup(keyboards::token_edit_group_picker_keyboard(token_id, page, &groups))
            .await?;
            Ok(true)
        }
        CallbackAction::ExecuteEditTokenGroup { token_id, group_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };

            let admin_id = q.from.id.0 as i64;
            crate::bot::handlers::state::clear_wizard_state(state, admin_id).await?;

            let new_group_id = if group_id == 0 { None } else { Some(group_id) };
            let updated = state.db.update_invite_token_group(token_id, new_group_id).await?;

            if !updated {
                ack_callback(bot, q.id.clone(), Some("Не удалось обновить группу токена"), true).await?;
                return Ok(true);
            }

            let group_name = if let Some(id) = new_group_id {
                state.db.get_user_group_by_id(id).await?
                    .map(|g| g.name)
                    .unwrap_or_else(|| format!("ID {}", id))
            } else {
                "без группы".to_string()
            };

            ack_callback(bot, q.id.clone(), Some(&format!("Группа: {}", group_name)), false).await?;

            let token = state.db.get_active_invite_token_by_id(token_id).await?;
            if let Some(token) = token {
                show_token_card(bot, chat_id, Some(message_id), state, &token, page).await?;
            }
            Ok(true)
        }
        CallbackAction::SetTokenExpirationDirect { token_id, days, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let _updated = state.db.update_invite_token_limits(token_id, Some(days), None, None).await?;
            ack_callback(bot, q.id.clone(), Some(&format!("Срок: {} дн.", days)), false).await?;
            if let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? {
                show_token_card(bot, chat_id, Some(message_id), state, &token, page).await?;
            }
            Ok(true)
        }
        CallbackAction::SetTokenMaxIpsDirect { token_id, count, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let _updated = state.db.update_invite_token_limits(token_id, None, count, None).await?;
            let ip_text = count.map(|c| c.to_string()).unwrap_or_else(|| "без лимита".to_string());
            ack_callback(bot, q.id.clone(), Some(&format!("IP: {}", ip_text)), false).await?;
            if let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? {
                show_token_card(bot, chat_id, Some(message_id), state, &token, page).await?;
            }
            Ok(true)
        }
        CallbackAction::SetTokenDataQuotaDirect { token_id, quota_gb, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let data_quota_bytes = quota_gb.map(|gb| gb * 1024 * 1024 * 1024);
            let _updated = state.db.update_invite_token_limits(token_id, None, None, data_quota_bytes).await?;
            let quota_text = quota_gb.map(|gb| format!("{} GB", gb)).unwrap_or_else(|| "безлимит".to_string());
            ack_callback(bot, q.id.clone(), Some(&format!("Квота: {}", quota_text)), false).await?;
            if let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? {
                show_token_card(bot, chat_id, Some(message_id), state, &token, page).await?;
            }
            Ok(true)
        }
        CallbackAction::ResetTokenLimits { token_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let _updated = state.db.update_invite_token_limits(token_id, None, None, None).await?;
            ack_callback(bot, q.id.clone(), Some("Лимиты сброшены"), false).await?;
            if let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? {
                show_token_card(bot, chat_id, Some(message_id), state, &token, page).await?;
            }
            Ok(true)
        }
        CallbackAction::PromptEditTokenLimits { token_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            bot.edit_message_text(
                chat_id,
                message_id,
                "Выберите новый срок доступа:",
            )
            .reply_markup(keyboards::token_edit_limits_keyboard(token_id, page))
            .await?;
            Ok(true)
        }
        CallbackAction::SendTokenStartLink { token_id } => {
            let Some((_, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Отправляю ссылку"), false).await?;
            send_token_start_link(bot, chat_id, state, token_id).await?;
            Ok(true)
        }
        CallbackAction::ConfirmTokenRevoke { token_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(token) = state.db.get_active_invite_token_by_id(token_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Токен уже недоступен"), true).await?;
                admin_show_token_list_page(bot, chat_id, state, page, Some(message_id)).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_token_revoke_confirm(bot, chat_id, message_id, &token, page).await?;
            Ok(true)
        }
        CallbackAction::ExecuteTokenRevoke { token_id, page } => {
            let Some(admin_id) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let revoked = state.db.revoke_invite_token_by_id(token_id).await?;
            let status_text = if revoked {
                tracing::info!("Admin {} revoked invite token #{}", admin_id, token_id);
                "Токен отозван".to_string()
            } else {
                "Токен не найден или уже недоступен".to_string()
            };
            ack_callback(bot, q.id.clone(), Some(&status_text), false).await?;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                admin_show_token_list_page(bot, chat_id, state, page, Some(message_id)).await?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
