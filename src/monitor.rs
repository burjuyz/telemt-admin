use crate::bot::handlers::BotState;
use crate::telemt_backend::TelemtMonitorSnapshot;
use teloxide::prelude::{Bot, ChatId, Requester};
use tokio::time::{Duration, MissedTickBehavior};

#[derive(Debug, Default, Clone)]
struct MonitorState {
    api_available: Option<bool>,
    health_status: Option<String>,
    accepting_new_connections: Option<bool>,
    me_runtime_ready: Option<bool>,
    upstream_unhealthy_total: Option<u64>,
    kdf_state: Option<String>,
    timeskew_state: Option<String>,
}

pub fn spawn_monitor(bot: Bot, state: BotState) {
    if !state.config.notifications.enabled || !state.config.telemt_api.enabled {
        tracing::info!("Фоновый монитор telemt отключён конфигом");
        return;
    }

    tokio::spawn(async move {
        let interval_secs = state.config.notifications.health_check_interval_secs.max(1);
        tracing::info!(
            interval_secs = interval_secs,
            "Запускаю фоновый монитор telemt"
        );
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut monitor_state = MonitorState::default();

        loop {
            ticker.tick().await;
            if let Err(error) = poll_once(&bot, &state, &mut monitor_state).await {
                tracing::warn!(error = %error, "Ошибка фонового мониторинга telemt");
            }
        }
    });
}

async fn poll_once(
    bot: &Bot,
    state: &BotState,
    monitor_state: &mut MonitorState,
) -> Result<(), anyhow::Error> {
    match state.telemt_backend.monitor_snapshot().await {
        Ok(Some(snapshot)) => {
            handle_snapshot(bot, state, monitor_state, snapshot).await?;
        }
        Ok(None) => {
            tracing::debug!("monitor_snapshot недоступен для текущего backend");
        }
        Err(error) => {
            if state.config.notifications.notify_on_health_change
                && monitor_state.api_available != Some(false)
            {
                notify_admins(
                    bot,
                    state,
                    format!("🚨 telemt control API недоступен\n\nПричина: {}", error),
                )
                .await;
            }
            monitor_state.api_available = Some(false);
        }
    }
    Ok(())
}

async fn handle_snapshot(
    bot: &Bot,
    state: &BotState,
    monitor_state: &mut MonitorState,
    snapshot: TelemtMonitorSnapshot,
) -> Result<(), anyhow::Error> {
    if monitor_state.api_available == Some(false) && state.config.notifications.notify_on_health_change
    {
        notify_admins(bot, state, "✅ telemt control API снова доступен".to_string()).await;
    }
    monitor_state.api_available = Some(true);

    if state.config.notifications.notify_on_health_change {
        if monitor_state.health_status.as_deref() != Some(snapshot.health_status.as_str())
            && let Some(previous) = monitor_state.health_status.as_deref()
        {
            notify_admins(
                bot,
                state,
                format!(
                    "ℹ️ telemt health изменился\n\nБыло: {}\nСтало: {}",
                    previous, snapshot.health_status
                ),
            )
            .await;
        }
        if monitor_state.accepting_new_connections == Some(true)
            && snapshot.accepting_new_connections == Some(false)
        {
            notify_admins(
                bot,
                state,
                "🚨 telemt перестал принимать новые соединения".to_string(),
            )
            .await;
        } else if monitor_state.accepting_new_connections == Some(false)
            && snapshot.accepting_new_connections == Some(true)
        {
            notify_admins(
                bot,
                state,
                "✅ telemt снова принимает новые соединения".to_string(),
            )
            .await;
        }
        if monitor_state.me_runtime_ready == Some(true) && snapshot.me_runtime_ready == Some(false) {
            notify_admins(bot, state, "🚨 ME runtime больше не готов".to_string()).await;
        } else if monitor_state.me_runtime_ready == Some(false)
            && snapshot.me_runtime_ready == Some(true)
        {
            notify_admins(bot, state, "✅ ME runtime снова готов".to_string()).await;
        }
    }

    if state.config.notifications.notify_on_runtime_alerts {
        let prev_upstream_bad = monitor_state.upstream_unhealthy_total.unwrap_or(0);
        let current_upstream_bad = snapshot.upstream_unhealthy_total.unwrap_or(0);
        if prev_upstream_bad == 0 && current_upstream_bad > 0 {
            notify_admins(
                bot,
                state,
                format!(
                    "🚨 В telemt появились unhealthy upstream\n\nКоличество: {}",
                    current_upstream_bad
                ),
            )
            .await;
        } else if prev_upstream_bad > 0 && current_upstream_bad == 0 {
            notify_admins(bot, state, "✅ Все upstream снова healthy".to_string()).await;
        }

        notify_state_recovery(
            bot,
            state,
            monitor_state.kdf_state.as_deref(),
            snapshot.me_selftest_kdf_state.as_deref(),
            "ME self-test KDF",
        )
        .await;
        notify_state_recovery(
            bot,
            state,
            monitor_state.timeskew_state.as_deref(),
            snapshot.me_selftest_timeskew_state.as_deref(),
            "ME self-test time skew",
        )
        .await;
    }

    monitor_state.health_status = Some(snapshot.health_status);
    monitor_state.accepting_new_connections = snapshot.accepting_new_connections;
    monitor_state.me_runtime_ready = snapshot.me_runtime_ready;
    monitor_state.upstream_unhealthy_total = snapshot.upstream_unhealthy_total;
    monitor_state.kdf_state = snapshot.me_selftest_kdf_state;
    monitor_state.timeskew_state = snapshot.me_selftest_timeskew_state;
    Ok(())
}

async fn notify_state_recovery(
    bot: &Bot,
    state: &BotState,
    previous: Option<&str>,
    current: Option<&str>,
    title: &str,
) {
    let prev_bad = previous.is_some_and(|value| value != "ok");
    let curr_bad = current.is_some_and(|value| value != "ok");
    if !prev_bad && curr_bad {
        notify_admins(
            bot,
            state,
            format!(
                "🚨 {} сообщает о проблеме\n\nТекущее состояние: {}",
                title,
                current.unwrap_or("unknown")
            ),
        )
        .await;
    } else if prev_bad && !curr_bad {
        notify_admins(bot, state, format!("✅ {} снова в состоянии ok", title)).await;
    }
}

async fn notify_admins(bot: &Bot, state: &BotState, text: String) {
    for admin_id in &state.config.admin_ids {
        if let Err(error) = bot.send_message(ChatId(*admin_id), text.clone()).await {
            tracing::warn!(
                admin_id = *admin_id,
                error = %error,
                "Не удалось отправить уведомление мониторинга"
            );
        }
    }
}
