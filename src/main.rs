//! telemt-admin — Telegram-бот для администрирования MTProxy telemt.

mod bot;
mod cli;
mod config;
mod db;
mod link;
mod service;
mod telemt_cfg;
mod update;

use clap::Parser;
use std::sync::Arc;
use teloxide::dispatching::Dispatcher;
use teloxide::prelude::*;
use tokio::sync::Mutex;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::CheckUpdate) => {
            update::run_check_update().await?;
            return Ok(());
        }
        Some(Commands::SelfUpdate) => {
            update::run_self_update().await?;
            return Ok(());
        }
        None => {}
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let config_path = cli.config_path();
    tracing::info!(
        "Starting telemt-admin with config {}",
        config_path.display()
    );

    let config = Arc::new(config::Config::load(&config_path)?);
    let token = config.bot_token()?;
    tracing::info!(
        admin_count = config.admin_ids.len(),
        db_path = %config.db_path.display(),
        telemt_config_path = %config.telemt_config_path.display(),
        service_name = %config.service_name,
        users_page_size = config.users_page_size,
        "Configuration loaded"
    );

    let db = Arc::new(db::Db::open(&config.db_path).await?);
    let telemt_cfg = Arc::new(telemt_cfg::TelemtConfig::new(&config.telemt_config_path));
    let service = service::ServiceController::new(&config.service_name);

    let bot = Bot::new(token);
    let bot_username = match bot.get_me().await {
        Ok(me) => me.user.username.clone(),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "Не удалось получить username бота через getMe"
            );
            None
        }
    };

    let state = bot::handlers::BotState {
        config,
        db,
        telemt_cfg,
        service,
        bot_username,
        awaiting_invite_users: Arc::new(Mutex::new(std::collections::HashSet::new())),
    };
    tracing::info!("Dispatcher initialized, bot is ready");

    Dispatcher::builder(bot, bot::handlers::schema())
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
