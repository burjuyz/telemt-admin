//! CLI-слой telemt-admin.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::DEFAULT_CONFIG_PATH;

#[derive(Parser)]
#[command(
    name = "telemt-admin",
    version,
    about = "Telegram-бот для администрирования MTProxy telemt",
    long_about = None
)]
pub struct Cli {
    /// Путь к конфигу telemt-admin
    #[arg(short = 'c', long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Путь к конфигу (позиционный, для совместимости со старым запуском)
    #[arg(value_name = "CONFIG", hide = true)]
    pub config_path: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Проверить наличие новой версии
    CheckUpdate,
    /// Обновить бинарник до последней версии (только Linux x86_64)
    SelfUpdate,
}

impl Cli {
    /// Возвращает путь к конфигу: приоритет у --config, затем позиционный аргумент, затем дефолт.
    pub fn config_path(&self) -> PathBuf {
        self.config
            .clone()
            .or_else(|| self.config_path.clone())
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH))
    }
}
