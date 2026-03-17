//! Управление systemd-сервисом telemt.

use std::process::Output;
#[cfg(unix)]
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct ServiceController {
    service_name: String,
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

impl ServiceController {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    #[cfg(unix)]
    async fn run_command(program: &str, args: Vec<String>) -> std::io::Result<Output> {
        let mut command = Command::new(program);
        command.args(args);
        command.output().await
    }

    /// После изменения конфига telemt: `systemctl kill -s HUP --kill-who=main`,
    /// при неудаче — restart.
    pub async fn notify_config_reloaded(&self) -> ServiceResult {
        #[cfg(unix)]
        {
            tracing::info!(
                service = %self.service_name,
                "Running systemctl kill -s HUP --kill-who=main"
            );
            let output = Self::run_command(
                "systemctl",
                vec![
                    "kill".to_string(),
                    "-s".to_string(),
                    "HUP".to_string(),
                    "--kill-who=main".to_string(),
                    self.service_name.clone(),
                ],
            )
            .await;

            match output {
                Ok(o) if o.status.success() => {
                    tracing::info!(
                        service = %self.service_name,
                        "telemt config reload: HUP sent via systemctl"
                    );
                    ServiceResult {
                        success: true,
                        stderr: "Конфиг перечитан (systemctl kill -s HUP --kill-who=main)"
                            .to_string(),
                    }
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
                    tracing::warn!(
                        service = %self.service_name,
                        stderr = %stderr,
                        "systemctl kill failed, falling back to restart"
                    );
                    self.restart().await
                }
                Err(e) => {
                    tracing::warn!(
                        service = %self.service_name,
                        error = %e,
                        "systemctl kill execution failed, falling back to restart"
                    );
                    self.restart().await
                }
            }
        }

        #[cfg(not(unix))]
        {
            tracing::debug!(service = %self.service_name, "non-Unix: restarting service");
            self.restart().await
        }
    }

    async fn run_systemctl(&self, action: &str) -> ServiceResult {
        tracing::info!(
            action = action,
            service = %self.service_name,
            "Running systemctl command"
        );
        #[cfg(unix)]
        let output = Self::run_command(
            "systemctl",
            vec![action.to_string(), self.service_name.clone()],
        )
        .await;

        #[cfg(not(unix))]
        let output: Result<Output, std::io::Error> =
            Err(std::io::Error::other("systemctl доступен только на Unix"));

        match output {
            Ok(o) => {
                let result = ServiceResult {
                    success: o.status.success(),
                    stderr: String::from_utf8_lossy(&o.stderr).trim().to_string(),
                };
                if result.success {
                    tracing::info!(
                        action = action,
                        service = %self.service_name,
                        "systemctl finished successfully"
                    );
                } else {
                    tracing::warn!(
                        action = action,
                        service = %self.service_name,
                        stderr = %result.stderr,
                        "systemctl returned non-zero status"
                    );
                }
                result
            }
            Err(e) => ServiceResult {
                success: false,
                stderr: {
                    tracing::error!(
                        action = action,
                        service = %self.service_name,
                        error = %e,
                        "Failed to execute systemctl"
                    );
                    format!("Ошибка запуска systemctl: {}", e)
                },
            },
        }
    }

    pub async fn start(&self) -> ServiceResult {
        self.run_systemctl("start").await
    }

    pub async fn stop(&self) -> ServiceResult {
        self.run_systemctl("stop").await
    }

    pub async fn restart(&self) -> ServiceResult {
        self.run_systemctl("restart").await
    }

    pub async fn reload(&self) -> ServiceResult {
        self.notify_config_reloaded().await
    }

    pub async fn status(&self) -> ServiceResult {
        self.run_systemctl("status").await
    }

    pub async fn summary(&self) -> ServiceSummary {
        #[cfg(unix)]
        {
            let output = Self::run_command(
                "systemctl",
                vec![
                    "show".to_string(),
                    self.service_name.clone(),
                    "--property=ActiveState".to_string(),
                    "--property=SubState".to_string(),
                    "--property=UnitFileState".to_string(),
                    "--property=MainPID".to_string(),
                    "--property=ExecMainStatus".to_string(),
                ],
            )
            .await;

            match output {
                Ok(o) if o.status.success() => {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    let mut active_state = String::from("unknown");
                    let mut sub_state = String::from("unknown");
                    let mut unit_file_state = String::from("unknown");
                    let mut main_pid = None;
                    let mut exec_main_status = None;

                    for line in stdout.lines() {
                        if let Some(value) = line.strip_prefix("ActiveState=") {
                            active_state = value.trim().to_string();
                        } else if let Some(value) = line.strip_prefix("SubState=") {
                            sub_state = value.trim().to_string();
                        } else if let Some(value) = line.strip_prefix("UnitFileState=") {
                            unit_file_state = value.trim().to_string();
                        } else if let Some(value) = line.strip_prefix("MainPID=") {
                            main_pid = value.trim().parse::<i64>().ok().filter(|pid| *pid > 0);
                        } else if let Some(value) = line.strip_prefix("ExecMainStatus=") {
                            exec_main_status = value
                                .trim()
                                .parse::<i64>()
                                .ok()
                                .filter(|status| *status >= 0);
                        }
                    }

                    ServiceSummary {
                        success: true,
                        active_state,
                        sub_state,
                        unit_file_state,
                        main_pid,
                        exec_main_status,
                        error: None,
                    }
                }
                Ok(o) => ServiceSummary {
                    success: false,
                    active_state: String::from("unknown"),
                    sub_state: String::from("unknown"),
                    unit_file_state: String::from("unknown"),
                    main_pid: None,
                    exec_main_status: None,
                    error: Some(String::from_utf8_lossy(&o.stderr).trim().to_string()),
                },
                Err(e) => ServiceSummary {
                    success: false,
                    active_state: String::from("unknown"),
                    sub_state: String::from("unknown"),
                    unit_file_state: String::from("unknown"),
                    main_pid: None,
                    exec_main_status: None,
                    error: Some(format!("Ошибка запуска systemctl show: {}", e)),
                },
            }
        }

        #[cfg(not(unix))]
        {
            ServiceSummary {
                success: false,
                active_state: String::from("unknown"),
                sub_state: String::from("unknown"),
                unit_file_state: String::from("unknown"),
                main_pid: None,
                exec_main_status: None,
                error: Some(String::from("Подробный статус доступен только на Unix")),
            }
        }
    }

    pub async fn recent_events(&self, limit: usize) -> ServiceEvents {
        #[cfg(unix)]
        {
            let output = Self::run_command(
                "journalctl",
                vec![
                    "-u".to_string(),
                    self.service_name.clone(),
                    "-n".to_string(),
                    limit.to_string(),
                    "--no-pager".to_string(),
                ],
            )
            .await;

            match output {
                Ok(o) if o.status.success() => {
                    let lines = String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(ToString::to_string)
                        .collect::<Vec<_>>();
                    ServiceEvents {
                        success: true,
                        lines,
                        error: None,
                    }
                }
                Ok(o) => ServiceEvents {
                    success: false,
                    lines: Vec::new(),
                    error: Some(String::from_utf8_lossy(&o.stderr).trim().to_string()),
                },
                Err(e) => ServiceEvents {
                    success: false,
                    lines: Vec::new(),
                    error: Some(format!("Ошибка запуска journalctl: {}", e)),
                },
            }
        }

        #[cfg(not(unix))]
        {
            let _ = limit;
            ServiceEvents {
                success: false,
                lines: Vec::new(),
                error: Some(String::from("journalctl доступен только на Unix")),
            }
        }
    }
}
