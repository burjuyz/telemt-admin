//! Общие типы ответов для управления процессом telemt.

#[derive(Debug, Clone, Default)]
pub struct RuntimeCapabilities {
    pub shows_systemd_unit: bool,
    pub shows_journal_tail: bool,
    pub can_start: bool,
    pub can_stop: bool,
    pub can_restart: bool,
    pub can_reload_config: bool,
}

#[derive(Debug)]
pub struct ServiceResult {
    pub success: bool,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct ServiceSummary {
    pub success: bool,
    pub active_state: String,
    pub sub_state: String,
    pub unit_file_state: String,
    pub main_pid: Option<i64>,
    pub exec_main_status: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ServiceEvents {
    pub success: bool,
    pub lines: Vec<String>,
    pub error: Option<String>,
}
