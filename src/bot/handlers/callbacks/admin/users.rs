use super::super::common::{ack_callback, admin_callback_target, start_wizard_from_callback};
use super::AdminActionResult;
use crate::bot::handlers::actions::{
    has_active_users, perform_hard_ban, send_user_start_link, show_user_card,
    user_limit_input_help,
};
use crate::bot::handlers::callback_data::CallbackAction;
use crate::bot::handlers::screens::{
    admin_show_users_page, admin_show_users_page_by_group, send_user_qr_to_admin, show_user_ban_confirm,
};
use crate::bot::handlers::shared::{callback_message_target, require_admin_callback};
use crate::bot::handlers::state::{BotState, clear_wizard_state, set_wizard_state, WizardState};
use crate::bot::keyboards::bulk_selection_actions_keyboard;
use teloxide::payloads::EditMessageTextSetters;
use teloxide::prelude::{Bot, CallbackQuery, Requester};
use teloxide::types::InputFile;

pub async fn handle(
    bot: &Bot,
    q: &CallbackQuery,
    state: &BotState,
    action: CallbackAction,
) -> AdminActionResult {
    match action {
        CallbackAction::ShowUsersPage { page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_users_page(bot, chat_id, state, page, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ShowUsersPageByGroup { page, group_id } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            admin_show_users_page_by_group(bot, chat_id, state, page, group_id, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ToggleUserSelection { tg_user_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            {
                let mut selected = state.selected_users.lock().unwrap();
                if selected.contains(&tg_user_id) {
                    selected.remove(&tg_user_id);
                } else {
                    selected.insert(tg_user_id);
                }
            }
            let count = state.selected_users.lock().unwrap().len();
            ack_callback(bot, q.id.clone(), Some(&format!("Выбрано: {}", count)), false).await?;
            admin_show_users_page(bot, chat_id, state, page, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ClearUserSelection => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            {
                let mut selected = state.selected_users.lock().unwrap();
                selected.clear();
            }
            ack_callback(bot, q.id.clone(), Some("Выбор очищен"), false).await?;
            admin_show_users_page(bot, chat_id, state, 1, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::ShowUserSelectionActions => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let count = state.selected_users.lock().unwrap().len();
            if count == 0 {
                ack_callback(bot, q.id.clone(), Some("Сначала выберите пользователей"), false).await?;
                return Ok(true);
            }
            ack_callback(bot, q.id.clone(), Some(&format!("Выбрано: {} пользователей", count)), false).await?;
            bot.edit_message_text(chat_id, message_id, format!("Выбрано пользователей: {}\n\nВыберите действие:", count))
                .reply_markup(bulk_selection_actions_keyboard())
                .await?;
            Ok(true)
        }
        CallbackAction::PromptUserLookup { page } => {
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptUserLookup { page },
                "Жду ID, @username или часть имени",
                "Отправьте Telegram ID, @username или часть имени/ника следующим сообщением.\n\nСписок можно оставить открытым.".to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::OpenUserCard { tg_user_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Открыта карточка"), false).await?;
            show_user_card(bot, chat_id, Some(message_id), &user, page, state).await?;
            Ok(true)
        }
        CallbackAction::PromptUserLimit {
            tg_user_id,
            page,
            field,
        } => {
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptUserLimit {
                    tg_user_id,
                    page,
                    field,
                },
                "Жду новое значение лимита",
                format!(
                    "Пользователь: {}\nИзмените параметр и отправьте новое значение следующим сообщением.\n\n{}",
                    crate::bot::handlers::format::user_display_name(&user),
                    user_limit_input_help(field)
                ),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::SendUserStartLink { tg_user_id } => {
            let Some((_, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Отправляю deep link"), false).await?;
            send_user_start_link(bot, chat_id, state, tg_user_id).await?;
            Ok(true)
        }
        CallbackAction::ViewUserQr { tg_user_id } => {
            let Some(_) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), Some("Отправляю ссылку и QR"), false).await?;
            send_user_qr_to_admin(bot, q, &user, state).await?;
            Ok(true)
        }
        CallbackAction::ConfirmUserBan { tg_user_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            ack_callback(bot, q.id.clone(), None, false).await?;
            show_user_ban_confirm(bot, chat_id, message_id, tg_user_id, page).await?;
            Ok(true)
        }
        CallbackAction::ExecuteUserBan { tg_user_id, page } => {
            let Some(_) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let status_text = perform_hard_ban(state, tg_user_id).await?;
            ack_callback(bot, q.id.clone(), Some(&status_text), false).await?;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                bot.send_message(chat_id, status_text).await?;
                admin_show_users_page(bot, chat_id, state, page, Some(message_id)).await?;
            }
            Ok(true)
        }
        CallbackAction::PromptDeleteUser => {
            let Some(admin_id) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            if !has_active_users(state).await? {
                clear_wizard_state(state, admin_id).await?;
                ack_callback(bot, q.id.clone(), Some("Активных пользователей нет"), true).await?;
                return Ok(true);
            }
            start_wizard_from_callback(
                bot,
                q,
                state,
                CallbackAction::PromptDeleteUser,
                "Жду Telegram ID",
                "Отправьте Telegram ID пользователя следующим сообщением.\n\nСообщение с кнопками можно оставить открытым.".to_string(),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::ExecuteDeleteUser { tg_user_id } => {
            let Some(_) = require_admin_callback(bot, q, state).await? else {
                return Ok(true);
            };
            let status_text = perform_hard_ban(state, tg_user_id).await?;
            ack_callback(bot, q.id.clone(), Some(&status_text), false).await?;
            if let Some((chat_id, message_id)) = callback_message_target(q) {
                bot.edit_message_text(chat_id, message_id, status_text)
                    .reply_markup(crate::bot::keyboards::admin_home_keyboard())
                    .await?;
            }
            Ok(true)
        }
        CallbackAction::UserGroupPicker { tg_user_id, page } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            let groups = state.db.list_user_groups().await?;
            let current = state
                .db
                .get_group_for_tg_user(tg_user_id)
                .await?
                .map(|g| g.name)
                .unwrap_or_else(|| "нет".to_string());
            let title = format!(
                "📁 Группа для {}\n\nТекущая: {}",
                crate::bot::handlers::format::user_display_name(&user),
                current
            );
            ack_callback(bot, q.id.clone(), None, false).await?;
            bot.edit_message_text(chat_id, message_id, title)
                .reply_markup(crate::bot::keyboards::user_group_picker_keyboard(
                    tg_user_id,
                    page,
                    &groups,
                ))
                .await?;
            Ok(true)
        }
        CallbackAction::AssignUserToGroup {
            tg_user_id,
            group_id,
            page,
        } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let Some(user) = state.db.get_active_user_by_tg_user(tg_user_id).await? else {
                ack_callback(bot, q.id.clone(), Some("Пользователь уже неактивен"), true).await?;
                return Ok(true);
            };
            let gid = if group_id == 0 {
                None
            } else {
                if state.db.get_user_group_by_id(group_id).await?.is_none() {
                    ack_callback(bot, q.id.clone(), Some("Группа не найдена"), true).await?;
                    return Ok(true);
                }
                Some(group_id)
            };
            state.db.set_user_group_membership(tg_user_id, gid).await?;
            ack_callback(bot, q.id.clone(), Some("Сохранено"), false).await?;
            show_user_card(bot, chat_id, Some(message_id), &user, page, state).await?;
            Ok(true)
        }
        CallbackAction::BulkAssignGroup { group_id } => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let selected: Vec<i64> = {
                let guard = state.selected_users.lock().unwrap();
                guard.iter().copied().collect()
            };
            if selected.is_empty() {
                ack_callback(bot, q.id.clone(), Some("Нет выбранных пользователей"), false).await?;
                return Ok(true);
            }
            for tg_user_id in &selected {
                state.db.set_user_group_membership(*tg_user_id, Some(group_id)).await?;
            }
            let count = selected.len();
            let group = state.db.get_user_group_by_id(group_id).await?;
            let group_name = group.map(|g| g.name).unwrap_or_else(|| "группы".to_string());
            {
                let mut guard = state.selected_users.lock().unwrap();
                guard.clear();
            }
            ack_callback(bot, q.id.clone(), Some(&format!("Добавлено {} пользователей в «{}»", count, group_name)), false).await?;
            admin_show_users_page(bot, chat_id, state, 1, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::BulkBanUsers => {
            let Some((_, chat_id, message_id)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let selected: Vec<i64> = {
                let guard = state.selected_users.lock().unwrap();
                guard.iter().copied().collect()
            };
            if selected.is_empty() {
                ack_callback(bot, q.id.clone(), Some("Нет выбранных пользователей"), false).await?;
                return Ok(true);
            }
            for tg_user_id in &selected {
                let _ = perform_hard_ban(state, *tg_user_id).await;
            }
            let count = selected.len();
            {
                let mut guard = state.selected_users.lock().unwrap();
                guard.clear();
            }
            ack_callback(bot, q.id.clone(), Some(&format!("Заблокировано: {} пользователей", count)), false).await?;
            admin_show_users_page(bot, chat_id, state, 1, Some(message_id)).await?;
            Ok(true)
        }
        CallbackAction::BulkSetUserLimit { field } => {
            let Some((admin_id, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let selected_count = {
                let guard = state.selected_users.lock().unwrap();
                guard.len()
            };
            if selected_count == 0 {
                ack_callback(bot, q.id.clone(), Some("Нет выбранных пользователей"), false).await?;
                return Ok(true);
            }
            let field_name = match field {
                crate::bot::handlers::callback_data::UserLimitField::MaxTcpConns => "TCP-лимит",
                crate::bot::handlers::callback_data::UserLimitField::MaxUniqueIps => "IP-лимит",
                crate::bot::handlers::callback_data::UserLimitField::DataQuotaBytes => "квота трафика",
                crate::bot::handlers::callback_data::UserLimitField::Expiration => "срок действия",
            };
            set_wizard_state(
                state,
                admin_id,
                WizardState::AdminSetUserLimitAwaitingValue {
                    tg_user_id: 0,
                    page: 1,
                    field,
                },
            ).await?;
            {
                let mut guard = state.selected_users.lock().unwrap();
                guard.clear();
            }
            ack_callback(bot, q.id.clone(), Some(&format!("Жду {} для {} пользователей", field_name, selected_count)), false).await?;
            bot.send_message(
                chat_id,
                format!("Введите {} для выбранных пользователей.", field_name),
            )
            .await?;
            Ok(true)
        }
        CallbackAction::ExportUsersCsv => {
            let Some((_, chat_id, _)) = admin_callback_target(bot, q, state).await? else {
                return Ok(true);
            };
            let users = state.db.list_active_users_page(1000, 0).await?;
            let count = users.len();
            let mut csv = String::from("id,username,display_name,telemt_username,created_at\n");
            for user in &users {
                let username = user.tg_username.clone().unwrap_or_default();
                let display_name = user.tg_display_name.clone().unwrap_or_default();
                let telemt_username = user.telemt_username.clone().unwrap_or_default();
                csv.push_str(&format!("{},{},{},{},{}\n", 
                    user.tg_user_id, 
                    username.replace(',', ";"),
                    display_name.replace(',', ";"),
                    telemt_username,
                    user.created_at
                ));
            }
            {
                let mut guard = state.selected_users.lock().unwrap();
                guard.clear();
            }
            ack_callback(bot, q.id.clone(), Some(&format!("Экспорт {} пользователей", count)), false).await?;
            let filename = format!("users_export_{}.csv", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
            bot.send_document(chat_id, InputFile::memory(csv.into_bytes()).file_name(filename))
                .await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
