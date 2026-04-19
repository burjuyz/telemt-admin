use super::callback_data::ServiceAction;
use super::format::{
    format_bytes_human, format_timestamp, render_invite_token_button_title,
    render_invite_token_card_text, render_user_card_text, usage_guide_text, user_display_name,
};
use super::shared::{HandlerResult, build_user_qr_png_bytes, callback_message_target};
use super::state::BotState;
use crate::db::{AdminActivity, AdminActivityKind, AdminStats, RequestStatus, SyncHealthSummary};
use crate::runtime::{RuntimeCapabilities, ServiceEvents, ServiceSummary};
use std::future::Future;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardMarkup, InputFile, KeyboardRemove, MessageId};

async fn upsert_screen(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    text: String,
    reply_markup: InlineKeyboardMarkup,
) -> HandlerResult {
    if let Some(message_id) = message_id {
        bot.edit_message_text(chat_id, message_id, text)
            .reply_markup(reply_markup)
            .await?;
    } else {
        bot.send_message(chat_id, text)
            .reply_markup(reply_markup)
            .await?;
    }
    Ok(())
}

struct PagedSelectorConfig {
    chat_id: ChatId,
    message_id: Option<MessageId>,
    total_items: i64,
    page_size: i64,
    requested_page: i64,
    empty_text: String,
    empty_keyboard: InlineKeyboardMarkup,
}

async fn render_paged_selector_screen<T, LoadFn, LoadFut, MapFn, TextFn, KeyboardFn>(
    bot: &Bot,
    config: PagedSelectorConfig,
    load_items: LoadFn,
    map_item: MapFn,
    text_builder: TextFn,
    keyboard_builder: KeyboardFn,
) -> HandlerResult
where
    LoadFn: FnOnce(i64, i64) -> LoadFut,
    LoadFut: Future<Output = Result<Vec<T>, anyhow::Error>>,
    MapFn: Fn(&T) -> (i64, String),
    TextFn: Fn(i64, i64, i64) -> String,
    KeyboardFn: Fn(&[(i64, String)], i64, i64) -> InlineKeyboardMarkup,
{
    if config.total_items <= 0 {
        upsert_screen(
            bot,
            config.chat_id,
            config.message_id,
            config.empty_text,
            config.empty_keyboard,
        )
        .await?;
        return Ok(());
    }

    let (page, total_pages, offset) =
        page_bounds(config.total_items, config.page_size, config.requested_page);
    let rows = load_items(config.page_size, offset).await?;
    let items: Vec<(i64, String)> = rows.iter().map(map_item).collect();
    let text = text_builder(config.total_items, page, total_pages);
    let keyboard = keyboard_builder(&items, page, total_pages);
    upsert_screen(bot, config.chat_id, config.message_id, text, keyboard).await?;
    Ok(())
}

fn page_bounds(total_items: i64, page_size: i64, requested_page: i64) -> (i64, i64, i64) {
    let total_pages = ((total_items + page_size - 1) / page_size).max(1);
    let page = requested_page.clamp(1, total_pages);
    let offset = (page - 1) * page_size;
    (page, total_pages, offset)
}

fn compact_line(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        trimmed.to_string()
    } else {
        format!(
            "{}...",
            trimmed
                .chars()
                .take(limit.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn service_action_title(action: ServiceAction) -> &'static str {
    match action {
        ServiceAction::Start => "запустить сервис",
        ServiceAction::Stop => "остановить сервис",
        ServiceAction::Restart => "перезапустить сервис",
        ServiceAction::Reload => "перечитать конфиг",
        ServiceAction::Status => "обновить статус",
    }
}

fn service_status_label(active_state: &str, sub_state: &str) -> String {
    match (active_state, sub_state) {
        ("active", "running") => "работает".to_string(),
        ("active", value) => value.to_string(),
        ("inactive", value) => value.to_string(),
        (value, _) => value.to_string(),
    }
}

fn admin_activity_summary(activity: &AdminActivity) -> String {
    match &activity.kind {
        AdminActivityKind::RequestApproved { request_id } => {
            format!("Заявка #{} одобрена", request_id)
        }
        AdminActivityKind::RequestRejected { request_id } => {
            format!("Заявка #{} отклонена", request_id)
        }
        AdminActivityKind::TokenCreated { token } => format!("Токен {} создан", token),
        AdminActivityKind::TokenRevoked { token } => format!("Токен {} отозван", token),
    }
}

pub struct ServicePanelData {
    pub notice: Option<String>,
    pub caps: RuntimeCapabilities,
    pub runtime_label: String,
    pub backend_mode: crate::telemt_backend::TelemtBackendMode,
    pub summary: ServiceSummary,
    pub service_events: ServiceEvents,
    pub admin_events: Vec<AdminActivity>,
    pub stats: AdminStats,
    pub active_tokens: i64,
    pub sync_health: SyncHealthSummary,
    pub telemt_stats: Option<crate::telemt_backend::TelemtStatsSummary>,
    pub telemt_stats_error: Option<String>,
    pub connections_summary: Option<crate::telemt_backend::TelemtConnectionsSummary>,
    pub connections_summary_error: Option<String>,
    pub runtime_snapshot: Option<crate::telemt_backend::TelemtRuntimeSnapshot>,
    pub runtime_snapshot_error: Option<String>,
}

fn render_service_panel_text(data: &ServicePanelData) -> String {
    let status_label = if data.caps.shows_systemd_unit {
        service_status_label(&data.summary.active_state, &data.summary.sub_state)
    } else {
        format!("{} · {}", data.summary.active_state, data.summary.sub_state)
    };
    let admin_version = env!("CARGO_PKG_VERSION");

    let mut lines = vec![
        "⚙️ Статус".to_string(),
        String::new(),
        format!("telemt-admin: v{} · бот активен", admin_version),
    ];

    lines.push(String::new());
    if data.caps.shows_systemd_unit {
        lines.push(format!("Юнит {}: {}", data.runtime_label, status_label));
        lines.push(format!(
            "Проверка systemd: {}",
            if data.summary.success {
                "OK"
            } else {
                "ошибка"
            }
        ));
    } else {
        lines.push(format!("Telemt ({}): {}", data.runtime_label, status_label));
        lines.push(format!(
            "Статус host-runtime: {}",
            if data.summary.success {
                "OK"
            } else {
                "ошибка"
            }
        ));
    }

    if let Some(notice) = data.notice.as_deref() {
        lines.push(format!("Действие: {}", notice));
    }

    if data.caps.shows_systemd_unit {
        lines.push(format!(
            "Unit: {} | PID: {}",
            data.summary.unit_file_state,
            data.summary
                .main_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "—".to_string())
        ));
    } else {
        lines.push("Unit/PID: не применимо (runtime external/none)".to_string());
    }
    lines.push(format!(
        "Пользователи: {} | Заявки: {} | Токены: {}",
        data.stats.approved, data.stats.pending, data.active_tokens
    ));
    lines.push(format!(
        "Sync: degraded {} | API {} | legacy {}",
        data.sync_health.degraded_users,
        data.sync_health.approved_via_control_api,
        data.sync_health.approved_via_legacy
    ));
    lines.push(format!(
        "Режим провижининга: {}",
        match data.backend_mode {
            crate::telemt_backend::TelemtBackendMode::LegacyFile => "файл + systemd",
            crate::telemt_backend::TelemtBackendMode::ControlApi => "control API",
        }
    ));

    if let Some(exec_status) = data.summary.exec_main_status {
        lines.push(format!("Код процесса: {}", exec_status));
    }
    if let Some(error) = &data.summary.error {
        lines.push(format!("Ошибка статуса: {}", compact_line(error, 90)));
    }

    if let Some(snapshot) = data.runtime_snapshot.as_ref() {
        lines.push(String::new());
        lines.push("Демон (control API)".to_string());
        lines.push(format!("Профиль: {}", snapshot.source.as_str()));
        lines.push(format!(
            "Версия: {} | Health: {} | read-only: {}",
            snapshot.build_version.as_deref().unwrap_or("—"),
            snapshot.health_status,
            if snapshot.api_read_only {
                "да"
            } else {
                "нет"
            }
        ));
        if let Some(mode) = &snapshot.transport_mode {
            lines.push(format!("Транспорт: {}", mode));
        }
        if let (Some(acc), Some(me_ready), Some(proxy), Some(route)) = (
            snapshot.accepting_new_connections,
            snapshot.me_runtime_ready,
            snapshot.use_middle_proxy,
            snapshot.route_mode.as_deref(),
        ) {
            lines.push(format!(
                "Маршрут: {} | middle proxy: {} | ME runtime: {} | приём соединений: {}",
                route,
                if proxy { "да" } else { "нет" },
                if me_ready { "да" } else { "нет" },
                if acc { "да" } else { "нет" }
            ));
        }
        if let (Some(cfg), Some(ok), Some(bad)) = (
            snapshot.upstream_configured_total,
            snapshot.upstream_healthy_total,
            snapshot.upstream_unhealthy_total,
        ) {
            lines.push(format!("Upstream: здоровых {} из {}", ok, cfg));
            if bad > 0 {
                lines.push(format!("⚠️ Нездоровых upstream: {}", bad));
            }
        }
        match snapshot.me_selftest_enabled {
            Some(true) => {
                let kdf = snapshot.me_selftest_kdf_state.as_deref().unwrap_or("—");
                let skew = snapshot
                    .me_selftest_timeskew_state
                    .as_deref()
                    .unwrap_or("—");
                lines.push(format!("ME self-test: KDF `{}` · время `{}`", kdf, skew));
            }
            Some(false) => {
                lines.push("ME self-test: данные пока недоступны (ME pool)".to_string());
            }
            None => {}
        }
        if let Some(startup_status) = snapshot.startup_status.as_deref() {
            let progress = snapshot
                .startup_progress_pct
                .map(|value| format!("{:.1}%", value))
                .unwrap_or_else(|| "—".to_string());
            lines.push(format!("Запуск: {} ({})", startup_status, progress));
        }
        if let Some(stage) = snapshot.startup_stage.as_deref() {
            lines.push(format!("Этап: {}", compact_line(stage, 60)));
        }
        if let Some(enabled) = snapshot.api_whitelist_enabled {
            let entries = snapshot
                .api_whitelist_entries
                .map(|value| value.to_string())
                .unwrap_or_else(|| "—".to_string());
            lines.push(format!(
                "API whitelist: {} ({})",
                if enabled { "вкл" } else { "выкл" },
                entries
            ));
        }
        if let Some(enabled) = snapshot.api_auth_header_enabled {
            lines.push(format!(
                "API auth header: {}",
                if enabled { "вкл" } else { "выкл" }
            ));
        }
        if let Some(revision) = snapshot.last_revision.as_deref() {
            lines.push(format!("Revision: {}", compact_line(revision, 24)));
        }
        lines.push(String::new());
        lines.push("События API:".to_string());
        if snapshot.events.is_empty() {
            lines.push("• нет данных".to_string());
        } else {
            for event in snapshot.events.iter().take(4) {
                lines.push(format!(
                    "• {} · {} · {}",
                    format_timestamp(event.ts_epoch_secs),
                    compact_line(&event.event_type, 28),
                    compact_line(&event.context, 42)
                ));
            }
        }
    } else if let Some(error) = data.runtime_snapshot_error.as_deref() {
        lines.push(String::new());
        lines.push("Демон (control API)".to_string());
        lines.push(format!(
            "Ошибка опроса runtime API: {}",
            compact_line(error, 90)
        ));
    }

    if let Some(summary) = data.telemt_stats.as_ref() {
        lines.push(String::new());
        lines.push("Нагрузка".to_string());
        lines.push(format!(
            "Uptime: {:.0} сек | users in config: {}",
            summary.uptime_seconds, summary.configured_users
        ));
        lines.push(format!(
            "Всего соединений: {} | bad: {} | handshake timeout: {}",
            summary.connections_total,
            summary.connections_bad_total,
            summary.handshake_timeouts_total
        ));
    } else if let Some(error) = data.telemt_stats_error.as_deref() {
        lines.push(String::new());
        lines.push(format!(
            "Нагрузка telemt: ошибка опроса ({})",
            compact_line(error, 90)
        ));
    }
    if let Some(connections) = data.connections_summary.as_ref() {
        lines.push(format!(
            "Live: {} | ME: {} | Direct: {} | active users: {}",
            connections.current_connections,
            connections.current_connections_me,
            connections.current_connections_direct,
            connections.active_users
        ));
    } else if let Some(error) = data.connections_summary_error.as_deref() {
        lines.push(format!(
            "Live connections: ошибка опроса ({})",
            compact_line(error, 90)
        ));
    }

    if !data.sync_health.top_sync_errors.is_empty() {
        lines.push(String::new());
        lines.push("Sync ошибки:".to_string());
        for item in data.sync_health.top_sync_errors.iter().take(3) {
            lines.push(format!("• {} · {}", item.code, item.affected_users));
        }
    }

    lines.push(String::new());
    lines.push(
        if data.caps.shows_journal_tail {
            "События сервиса:"
        } else {
            "События сервиса (journal недоступен в этом runtime):"
        }
        .to_string(),
    );
    if data.service_events.lines.is_empty() {
        lines.push(
            data.service_events
                .error
                .as_deref()
                .map(|error| format!("• {}", compact_line(error, 90)))
                .unwrap_or_else(|| "• нет данных".to_string()),
        );
    } else {
        if !data.service_events.success {
            lines.push("• журнал прочитан частично".to_string());
        }
        for line in data.service_events.lines.iter().take(3) {
            lines.push(format!("• {}", compact_line(line, 90)));
        }
    }

    lines.push(String::new());
    lines.push("Действия админа:".to_string());
    if data.admin_events.is_empty() {
        lines.push("• пока нет событий".to_string());
    } else {
        for item in data.admin_events.iter().take(4) {
            lines.push(format!(
                "• {} · {}",
                format_timestamp(item.timestamp),
                compact_line(&admin_activity_summary(item), 70)
            ));
        }
    }

    lines.join("\n")
}

pub async fn send_text_with_keyboard_removed(
    bot: &Bot,
    chat_id: ChatId,
    text: impl Into<String>,
) -> HandlerResult {
    bot.send_message(chat_id, text.into())
        .reply_markup(KeyboardRemove::new())
        .await?;
    Ok(())
}

pub async fn show_admin_home(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let text = "Панель администратора\n\nВыберите раздел ниже.";
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text.to_string(),
        crate::bot::keyboards::admin_home_keyboard(),
    )
    .await
}

pub async fn show_user_home(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
    user_id: i64,
) -> HandlerResult {
    let (text, keyboard) = if let Some(existing) = state.db.get_request_by_tg_user(user_id).await? {
        match existing.status {
            RequestStatus::Approved => {
                (
                    "✅ Доступ открыт.\n\nНажмите «Моя ссылка», чтобы получить ссылку на прокси.".to_string(),
                    crate::bot::keyboards::user_home_keyboard(),
                )
            }
            RequestStatus::Pending => {
                (
                    "⏳ Заявка на рассмотрении.\n\nДождитесь решения администратора.\nДля ускорения — отправьте invite-токен, если он у вас есть.".to_string(),
                    crate::bot::keyboards::user_pending_keyboard(),
                )
            }
            RequestStatus::Rejected => {
                (
                    "❌ Заявка отклонена.\n\nЕсли у вас есть новый invite-токен, введите его.".to_string(),
                    crate::bot::keyboards::user_pending_keyboard(),
                )
            }
            RequestStatus::Deleted => {
                (
                    "🚫 Доступ отозван.\n\nДля новой регистрации введите новый invite-токен.".to_string(),
                    crate::bot::keyboards::user_pending_keyboard(),
                )
            }
        }
    } else {
        (
            "🔒 Чтобы получить доступ, введите invite-токен.".to_string(),
            crate::bot::keyboards::user_pending_keyboard(),
        )
    };

    upsert_screen(bot, chat_id, message_id, text, keyboard).await
}

pub async fn show_usage_guide(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        usage_guide_text().to_string(),
        crate::bot::keyboards::guide_keyboard(),
    )
    .await
}

/// Показать ссылку на прокси (editable message).
/// Если пользователь approved — показывает ссылку, если нет — предлагает регистрацию.
pub async fn show_user_link_screen(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
    user_id: i64,
) -> HandlerResult {
    let maybe = state.db.get_approved(user_id).await?;
    if let Some((telemt_user, secret)) = maybe {
        let secret_opt = (!secret.is_empty()).then_some(secret.as_str());
        let link = state
            .telemt_backend
            .build_user_link(telemt_user.as_str(), secret_opt)
            .await?;
        let text = state.config.bot_messages.user_link_text(&link);
        upsert_screen(
            bot,
            chat_id,
            message_id,
            text,
            crate::bot::keyboards::user_home_keyboard(),
        )
        .await
    } else {
        let text = "🔒 У вас нет доступа к прокси.\n\nВведите invite-токен, чтобы получить доступ."
            .to_string();
        upsert_screen(
            bot,
            chat_id,
            message_id,
            text,
            crate::bot::keyboards::user_pending_keyboard(),
        )
        .await
    }
}

pub async fn show_token_menu(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
) -> HandlerResult {
    let text = "Управление invite-токенами\n\n\
        Ссылка действует ограниченное время и ограничена числом активаций; это не срок подписки пользователя в telemt.\n\n\
        Выберите действие.";
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text.to_string(),
        crate::bot::keyboards::token_menu_keyboard(state.config.security.allow_auto_approve_tokens),
    )
    .await
}

pub async fn admin_show_token_list_page(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    requested_page: i64,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let total_tokens = state.db.count_active_invite_tokens().await?;
    let tokens_page_size = state.config.users_page_size.max(1);
    render_paged_selector_screen(
        bot,
        PagedSelectorConfig {
            chat_id,
            message_id,
            total_items: total_tokens,
            page_size: tokens_page_size,
            requested_page,
            empty_text: "🎟 Токены\n\nАктивных invite-токенов нет.\n\
                (Срок в параметрах токена — это срок ссылки, не пользователя в telemt.)"
                .to_string(),
            empty_keyboard: crate::bot::keyboards::token_menu_keyboard(
                state.config.security.allow_auto_approve_tokens,
            ),
        },
        |limit, offset| state.db.list_active_invite_tokens_page(limit, offset),
        |token| (token.id, render_invite_token_button_title(token)),
        |total, page, total_pages| {
            format!(
                "🎟 Токены · {}\nСтраница: {}/{}\n\n\
                 В списке — срок действия invite-ссылки, не подписки пользователя.\n\n\
                 Выберите токен.",
                total, page, total_pages
            )
        },
        crate::bot::keyboards::token_list_keyboard,
    )
    .await
}

pub async fn show_token_card(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    token: &crate::db::InviteToken,
    page: i64,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        render_invite_token_card_text(token),
        crate::bot::keyboards::token_card_keyboard(token.id, page),
    )
    .await
}

pub async fn show_token_revoke_confirm(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    token: &crate::db::InviteToken,
    page: i64,
) -> HandlerResult {
    bot.edit_message_text(
        chat_id,
        message_id,
        format!(
            "Отозвать invite-токен {}?\n\nПосле этого его нельзя будет использовать для регистрации.",
            token.token
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_token_revoke_keyboard(
        token.id, page,
    ))
    .await?;
    Ok(())
}

pub async fn show_delete_user_confirm(
    bot: &Bot,
    chat_id: ChatId,
    tg_user_id: i64,
) -> HandlerResult {
    bot.send_message(
        chat_id,
        format!(
            "Удалить пользователя с Telegram ID {}?\n\nДействие деактивирует пользователя в БД и удалит его из telemt-конфига.",
            tg_user_id
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_delete_keyboard(tg_user_id))
    .await?;
    Ok(())
}

pub async fn admin_show_pending_requests_page(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    requested_page: i64,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let total_pending = state.db.count_pending_requests().await?;
    let requests_page_size = state.config.users_page_size.max(1);
    render_paged_selector_screen(
        bot,
        PagedSelectorConfig {
            chat_id,
            message_id,
            total_items: total_pending,
            page_size: requests_page_size,
            requested_page,
            empty_text: "📥 Заявки\n\nНовых заявок нет.".to_string(),
            empty_keyboard: crate::bot::keyboards::admin_home_keyboard(),
        },
        |limit, offset| state.db.list_pending_requests_page(limit, offset),
        |req| {
            (
                req.id,
                format!("📋 #{} · {}", req.id, user_display_name(req)),
            )
        },
        |total, page, total_pages| {
            format!(
                "📥 Заявки · {}\nСтраница: {}/{}\n\nВыберите заявку.",
                total, page, total_pages
            )
        },
        crate::bot::keyboards::pending_requests_keyboard,
    )
    .await
}

pub async fn show_pending_request_card(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    request: &crate::db::RegistrationRequest,
    page: i64,
) -> HandlerResult {
    let invite_line = request
        .invite_token_id
        .map(|id| format!("\n🎟 ID ссылки (invite): {}", id))
        .unwrap_or_default();
    let text = format!(
        "📋 Заявка #{}\n\n\
         👤 {}\n\
         🆔 {}\n\
         📱 {}\n\
         📅 {}{}",
        request.id,
        user_display_name(request),
        request.tg_user_id,
        request
            .tg_username
            .as_deref()
            .map(|username| format!("@{}", username))
            .unwrap_or_else(|| "—".to_string()),
        format_timestamp(request.created_at),
        invite_line,
    );
    bot.edit_message_text(chat_id, message_id, text)
        .reply_markup(crate::bot::keyboards::pending_request_card_keyboard(
            request.id, page,
        ))
        .await?;
    Ok(())
}

pub async fn show_user_ban_confirm(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    tg_user_id: i64,
    page: i64,
) -> HandlerResult {
    bot.edit_message_text(
        chat_id,
        message_id,
        format!(
            "Удалить пользователя {}?\n\nЭто действие уберёт запись из telemt и деактивирует доступ в БД.",
            tg_user_id
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_user_ban_keyboard(
        tg_user_id, page,
    ))
    .await?;
    Ok(())
}

pub async fn admin_show_users_page(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    requested_page: i64,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let total_users = state.db.count_active_users().await?;
    let users_page_size = state.config.users_page_size.max(1);
    let selected_users = state.selected_users.lock().unwrap().clone();
    render_paged_selector_screen(
        bot,
        PagedSelectorConfig {
            chat_id,
            message_id,
            total_items: total_users,
            page_size: users_page_size,
            requested_page,
            empty_text:
                "👥 Пользователи\n\nАктивных пользователей нет.\n\nМожно создать нового пользователя."
                    .to_string(),
            empty_keyboard: crate::bot::keyboards::users_page_keyboard_empty(1, None),
        },
        |limit, offset| state.db.list_active_users_page(limit, offset),
        |user| {
            let display_name = user_display_name(user);
            let short = if display_name.chars().count() > 40 {
                format!("{}...", display_name.chars().take(37).collect::<String>())
            } else {
                display_name
            };
            (user.tg_user_id, format!("{} (id {})", short, user.tg_user_id))
        },
        |total, page, total_pages| {
            let selected_count = selected_users.len();
            let header = if selected_count > 0 {
                format!("👥 Пользователи · {} (выбрано: {})\nСтраница: {}/{}", total, selected_count, page, total_pages)
            } else {
                format!("👥 Пользователи · {}\nСтраница: {}/{}", total, page, total_pages)
            };
            format!("{}\n\nВыберите пользователя (⬜ - выбрать).", header)
        },
        |users, page, total_pages| {
            crate::bot::keyboards::users_page_keyboard(users, page, total_pages, None, &selected_users)
        },
    )
    .await
}

pub async fn admin_show_users_page_by_group(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    requested_page: i64,
    group_id: i64,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let group = state.db.get_user_group_by_id(group_id).await?;
    let group_name = group
        .map(|g| g.name)
        .unwrap_or_else(|| "Группа".to_string());

    let total_users = state.db.count_users_in_group(group_id).await?;
    let users_page_size = state.config.users_page_size.max(1);
    let selected_users = state.selected_users.lock().unwrap().clone();

    render_paged_selector_screen(
        bot,
        PagedSelectorConfig {
            chat_id,
            message_id,
            total_items: total_users,
            page_size: users_page_size,
            requested_page,
            empty_text: format!(
                "👥 Пользователи · [{}]\n\nПользователей в группе нет.",
                group_name
            ),
            empty_keyboard: crate::bot::keyboards::users_page_keyboard_empty(1, Some(group_id)),
        },
        |limit, offset| state.db.list_users_in_group(group_id, limit, offset),
        |&tg_user_id| (tg_user_id, format!("id {}", tg_user_id)),
        |total, page, total_pages| {
            let selected_count = selected_users.len();
            let header = if selected_count > 0 {
                format!(
                    "👥 Пользователи · [{}]\nУчастников: {} (выбрано: {}) | Стр: {}/{}",
                    group_name, total, selected_count, page, total_pages
                )
            } else {
                format!(
                    "👥 Пользователи · [{}]\nУчастников: {} | Страница: {}/{}",
                    group_name, total, page, total_pages
                )
            };
            format!("{}\n\nВыберите пользователя (⬜ - выбрать).", header)
        },
        |users, page, total_pages| {
            crate::bot::keyboards::users_page_keyboard(
                users,
                page,
                total_pages,
                Some(group_id),
                &selected_users,
            )
        },
    )
    .await
}

pub async fn admin_show_stats(
    bot: &Bot,
    chat_id: ChatId,
    state: &BotState,
    message_id: Option<MessageId>,
) -> HandlerResult {
    let stats = state.db.admin_stats().await?;
    let caps = state.telemt_runtime.capabilities();
    let summary = state.telemt_runtime.summary().await;
    let admin_events = state.db.list_recent_admin_activities(4).await?;
    let telemt_stats = state.telemt_backend.stats_summary().await.ok().flatten();
    let connections_summary = state
        .telemt_backend
        .connections_summary(3)
        .await
        .ok()
        .flatten();
    let status_label = if caps.shows_systemd_unit {
        service_status_label(&summary.active_state, &summary.sub_state)
    } else {
        format!("{} · {}", summary.active_state, summary.sub_state)
    };

    let mut lines = vec![
        "📊 Сводка состояния".to_string(),
        String::new(),
        format!("Сервис: {}", state.telemt_runtime.display_label()),
        format!("Статус: {}", status_label),
        format!(
            "{}: {}",
            if caps.shows_systemd_unit {
                "Проверка systemd"
            } else {
                "Host-runtime"
            },
            if summary.success {
                "OK"
            } else {
                "Ошибка"
            }
        ),
    ];
    if caps.shows_systemd_unit {
        lines.push(format!(
            "Unit: {} | PID: {}",
            summary.unit_file_state,
            summary
                .main_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "—".to_string())
        ));
    } else {
        lines.push("Unit/PID: не применимо (runtime external/none)".to_string());
    }

    if let Some(exec_status) = summary.exec_main_status {
        lines.push(format!("Код процесса: {}", exec_status));
    }
    if let Some(error) = &summary.error {
        lines.push(format!("Ошибка статуса: {}", compact_line(error, 90)));
    }

    lines.push(String::new());
    lines.push("Доступ и заявки:".to_string());
    lines.push(format!("• Активные пользователи: {}", stats.approved));
    lines.push(format!("• Заявки в ожидании: {}", stats.pending));
    lines.push(format!("• Отклонённые заявки: {}", stats.rejected));
    lines.push(format!("• Отозванные доступы: {}", stats.deleted));
    lines.push(format!("• Всего записей: {}", stats.total));

    lines.push(String::new());
    lines.push("Invite-токены:".to_string());
    lines.push(format!("• Активные: {}", stats.tokens_active));
    lines.push(format!(
        "• Активные ручные / авто: {} / {}",
        stats.tokens_manual_active, stats.tokens_auto_active
    ));
    lines.push(format!("• Отозванные: {}", stats.tokens_revoked));
    lines.push(format!("• Истёкшие: {}", stats.tokens_expired));
    lines.push(format!("• Исчерпанные: {}", stats.tokens_exhausted));
    lines.push(format!("• Всего создано: {}", stats.tokens_total));

    lines.push(String::new());
    lines.push("Live telemt:".to_string());
    if let Some(stats_summary) = telemt_stats.as_ref() {
        lines.push(format!(
            "• Uptime: {:.0} s | configured users: {}",
            stats_summary.uptime_seconds, stats_summary.configured_users
        ));
        lines.push(format!(
            "• Connections total / bad: {} / {}",
            stats_summary.connections_total, stats_summary.connections_bad_total
        ));
        lines.push(format!(
            "• Handshake timeouts: {}",
            stats_summary.handshake_timeouts_total
        ));
    } else {
        lines.push("• stats summary: нет данных".to_string());
    }
    if let Some(live) = connections_summary.as_ref() {
        lines.push(format!(
            "• Live connections: {} | ME: {} | Direct: {} | active users: {}",
            live.current_connections,
            live.current_connections_me,
            live.current_connections_direct,
            live.active_users
        ));
        if let Some(top) = live.top_by_connections.first() {
            lines.push(format!(
                "• Top TCP: {} ({} conn, {})",
                top.username,
                top.current_connections,
                format_bytes_human(top.total_octets)
            ));
        }
        if let Some(top) = live.top_by_throughput.first() {
            lines.push(format!(
                "• Top traffic: {} ({})",
                top.username,
                format_bytes_human(top.total_octets)
            ));
        }
        let mut alerts = Vec::new();
        if !live.top_by_connections.is_empty() {
            for user in &live.top_by_connections {
                if user.current_connections >= 10 {
                    alerts.push(format!(
                        "TCP spike: {} ({})",
                        user.username, user.current_connections
                    ));
                }
            }
        }
        if !live.top_by_throughput.is_empty() {
            for user in &live.top_by_throughput {
                if user.total_octets >= 1024_u64.pow(3) {
                    alerts.push(format!(
                        "traffic spike: {} ({})",
                        user.username,
                        format_bytes_human(user.total_octets)
                    ));
                }
            }
        }
        if alerts.is_empty() {
            lines.push("• Аномалии: не обнаружены".to_string());
        } else {
            lines.push(format!("• Аномалии: {}", alerts.join("; ")));
        }
    } else {
        lines.push("• connections summary: нет данных".to_string());
    }

    lines.push(String::new());
    lines.push("Недавняя активность:".to_string());
    if admin_events.is_empty() {
        lines.push("• пока нет событий".to_string());
    } else {
        for item in admin_events.iter().take(4) {
            lines.push(format!(
                "• {} · {}",
                format_timestamp(item.timestamp),
                compact_line(&admin_activity_summary(item), 70)
            ));
        }
    }

    let text = lines.join("\n");
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::stats_keyboard(),
    )
    .await
}

pub async fn admin_show_service_panel_screen(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    data: ServicePanelData,
) -> HandlerResult {
    let text = render_service_panel_text(&data);
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::service_control_buttons(&data.caps),
    )
    .await
}

pub async fn show_service_action_confirm(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    action: ServiceAction,
) -> HandlerResult {
    bot.edit_message_text(
        chat_id,
        message_id,
        format!(
            "Подтвердить действие: {}?\n\nЭто может временно прервать доступ пользователей.",
            service_action_title(action)
        ),
    )
    .reply_markup(crate::bot::keyboards::confirm_service_action_keyboard(
        action,
    ))
    .await?;
    Ok(())
}

pub async fn send_user_qr_to_admin(
    bot: &Bot,
    q: &CallbackQuery,
    user: &crate::db::RegistrationRequest,
    state: &BotState,
) -> Result<(), anyhow::Error> {
    let Some(telemt_username) = user.telemt_username.as_deref() else {
        return Err(anyhow::anyhow!("Не найден telemt username пользователя"));
    };

    let secret_opt = user.secret.as_deref().filter(|s| !s.is_empty());
    let link = state
        .telemt_backend
        .build_user_link(telemt_username, secret_opt)
        .await?;
    let qr_png = build_user_qr_png_bytes(&link)?;
    let caption = format!(
        "👤 {} ({})\n\n🔗 {}",
        user_display_name(user),
        user.tg_user_id,
        link
    );

    if let Some((chat_id, _)) = callback_message_target(q) {
        bot.send_photo(
            chat_id,
            InputFile::memory(qr_png).file_name(format!("telemt-proxy-{}.png", user.tg_user_id)),
        )
        .caption(caption)
        .await?;
    }
    Ok(())
}

pub async fn show_user_card_screen(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    user: &crate::db::RegistrationRequest,
    runtime_info: Option<crate::telemt_backend::TelemtUserInfo>,
    page: i64,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        render_user_card_text(user, runtime_info.as_ref()),
        crate::bot::keyboards::user_card_keyboard(user.tg_user_id, page),
    )
    .await
}

fn render_connections_summary_text(
    summary: Option<&crate::telemt_backend::TelemtConnectionsSummary>,
    error: Option<&str>,
) -> String {
    match summary {
        Some(summary) => {
            let mut lines = vec![
                "📈 Top пользователей".to_string(),
                String::new(),
                format!(
                    "Live connections: {} | ME: {} | Direct: {} | active users: {}",
                    summary.current_connections,
                    summary.current_connections_me,
                    summary.current_connections_direct,
                    summary.active_users
                ),
                String::new(),
                "Топ по соединениям:".to_string(),
            ];
            if summary.top_by_connections.is_empty() {
                lines.push("• нет данных".to_string());
            } else {
                for user in summary.top_by_connections.iter().take(5) {
                    lines.push(format!(
                        "• {} · conns {} · traffic {}",
                        user.username,
                        user.current_connections,
                        format_bytes_human(user.total_octets)
                    ));
                }
            }
            lines.push(String::new());
            lines.push("Топ по трафику:".to_string());
            if summary.top_by_throughput.is_empty() {
                lines.push("• нет данных".to_string());
            } else {
                for user in summary.top_by_throughput.iter().take(5) {
                    lines.push(format!(
                        "• {} · traffic {} · conns {}",
                        user.username,
                        format_bytes_human(user.total_octets),
                        user.current_connections
                    ));
                }
            }
            lines.join("\n")
        }
        None => {
            let mut text =
                "📈 Top пользователей\n\nRuntime endpoint недоступен или выключен в telemt API."
                    .to_string();
            if let Some(error) = error {
                text.push_str("\n\nПричина: ");
                text.push_str(&compact_line(error, 90));
            }
            text
        }
    }
}

pub async fn admin_show_connections_summary_screen(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    summary: Option<crate::telemt_backend::TelemtConnectionsSummary>,
    summary_error: Option<String>,
) -> HandlerResult {
    upsert_screen(
        bot,
        chat_id,
        message_id,
        render_connections_summary_text(summary.as_ref(), summary_error.as_deref()),
        crate::bot::keyboards::connections_summary_keyboard(),
    )
    .await
}

pub async fn admin_show_groups_menu(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
    selection_mode: bool,
) -> HandlerResult {
    let groups = state.db.list_user_groups().await?;
    let text = if groups.is_empty() {
        if selection_mode {
            "📁 Выберите группу для выбранных пользователей\n\nПока нет ни одной группы."
                .to_string()
        } else {
            "📁 Группы пользователей\n\nПока нет ни одной группы. Нажмите «Новая группа»."
                .to_string()
        }
    } else {
        if selection_mode {
            "📁 Выберите группу для выбранных пользователей".to_string()
        } else {
            "📁 Группы пользователей\n\nВыберите группу или создайте новую.".to_string()
        }
    };
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::groups_menu_keyboard(&groups, selection_mode),
    )
    .await
}

pub async fn admin_show_group_card(
    bot: &Bot,
    chat_id: ChatId,
    message_id: Option<MessageId>,
    state: &BotState,
    group: &crate::db::UserGroup,
) -> HandlerResult {
    let n = state.db.count_group_members(group.id).await?;
    let created_line = format_timestamp(group.created_at);
    let exp_line = match group.expires_at {
        Some(ts) => format!(
            "\nОбщий срок группы: {}\nUnix timestamp: {}",
            format_timestamp(ts),
            ts
        ),
        None => "\nОбщий срок группы: не задан.".to_string(),
    };
    let text = format!(
        "📁 Группа: {}\nID: {}\nСоздана: {}\nУчастников: {}{}\n\n\
         «Задать/изменить срок» обновит общий срок группы через UI.\n\
         «Снять срок» очистит общий срок группы.\n\
         «Отключить всех» удалит пользователей из telemt и локальной БД, затем удалит группу.\n\
         «Применить срок» выставит всем участникам `expiration` из RFC3339, вычисленного из unix-срока группы.",
        group.name, group.id, created_line, n, exp_line
    );
    upsert_screen(
        bot,
        chat_id,
        message_id,
        text,
        crate::bot::keyboards::group_card_keyboard(group.id),
    )
    .await
}
