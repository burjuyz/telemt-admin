#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Reload,
    Status,
}

impl ServiceAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Reload => "reload",
            Self::Status => "status",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "start" => Some(Self::Start),
            "stop" => Some(Self::Stop),
            "restart" => Some(Self::Restart),
            "reload" => Some(Self::Reload),
            "status" => Some(Self::Status),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallbackAction {
    ShowAdminHome,
    ShowUserHome,
    ShowUserLink,
    ShowUsageGuide,
    PromptInviteToken,
    CancelWizard,
    ShowPendingRequests,
    OpenPendingRequest { request_id: i64 },
    ShowUsersPage { page: i64 },
    OpenUserCard { tg_user_id: i64, page: i64 },
    ViewUserQr { tg_user_id: i64 },
    ConfirmUserBan { tg_user_id: i64, page: i64 },
    ExecuteUserBan { tg_user_id: i64, page: i64 },
    ShowStats,
    ShowServicePanel,
    RunServiceAction { action: ServiceAction },
    ShowTokenMenu,
    PromptTokenCreate { auto_approve: bool },
    ShowTokenList,
    PromptTokenRevoke,
    PromptCreateUser,
    PromptDeleteUser,
    ConfirmDeleteUser { tg_user_id: i64 },
    ExecuteDeleteUser { tg_user_id: i64 },
    ApproveRequest { request_id: i64 },
    RejectRequest { request_id: i64 },
}

impl CallbackAction {
    pub fn encode(&self) -> String {
        match self {
            Self::ShowAdminHome => "v1|admin|home".to_string(),
            Self::ShowUserHome => "v1|user|home".to_string(),
            Self::ShowUserLink => "v1|user|link".to_string(),
            Self::ShowUsageGuide => "v1|user|guide".to_string(),
            Self::PromptInviteToken => "v1|user|invite".to_string(),
            Self::CancelWizard => "v1|wizard|cancel".to_string(),
            Self::ShowPendingRequests => "v1|admin|pending".to_string(),
            Self::OpenPendingRequest { request_id } => {
                format!("v1|admin|pending|open|{request_id}")
            }
            Self::ShowUsersPage { page } => format!("v1|admin|users|page|{page}"),
            Self::OpenUserCard { tg_user_id, page } => {
                format!("v1|admin|user|open|{tg_user_id}|{page}")
            }
            Self::ViewUserQr { tg_user_id } => format!("v1|admin|user|view|{tg_user_id}"),
            Self::ConfirmUserBan { tg_user_id, page } => {
                format!("v1|admin|user|ban_confirm|{tg_user_id}|{page}")
            }
            Self::ExecuteUserBan { tg_user_id, page } => {
                format!("v1|admin|user|ban_execute|{tg_user_id}|{page}")
            }
            Self::ShowStats => "v1|admin|stats".to_string(),
            Self::ShowServicePanel => "v1|admin|service".to_string(),
            Self::RunServiceAction { action } => {
                format!("v1|admin|service|run|{}", action.as_str())
            }
            Self::ShowTokenMenu => "v1|admin|token".to_string(),
            Self::PromptTokenCreate { auto_approve } => {
                let auto_approve = if *auto_approve { 1 } else { 0 };
                format!("v1|admin|token|create|{auto_approve}")
            }
            Self::ShowTokenList => "v1|admin|token|list".to_string(),
            Self::PromptTokenRevoke => "v1|admin|token|revoke".to_string(),
            Self::PromptCreateUser => "v1|admin|create".to_string(),
            Self::PromptDeleteUser => "v1|admin|delete".to_string(),
            Self::ConfirmDeleteUser { tg_user_id } => {
                format!("v1|admin|delete|confirm|{tg_user_id}")
            }
            Self::ExecuteDeleteUser { tg_user_id } => {
                format!("v1|admin|delete|execute|{tg_user_id}")
            }
            Self::ApproveRequest { request_id } => format!("v1|req|approve|{request_id}"),
            Self::RejectRequest { request_id } => format!("v1|req|reject|{request_id}"),
        }
    }

    pub fn decode(data: &str) -> Option<Self> {
        Self::decode_v1(data).or_else(|| Self::decode_legacy(data))
    }

    fn decode_v1(data: &str) -> Option<Self> {
        let parts: Vec<&str> = data.split('|').collect();
        match parts.as_slice() {
            ["v1", "admin", "home"] => Some(Self::ShowAdminHome),
            ["v1", "user", "home"] => Some(Self::ShowUserHome),
            ["v1", "user", "link"] => Some(Self::ShowUserLink),
            ["v1", "user", "guide"] => Some(Self::ShowUsageGuide),
            ["v1", "user", "invite"] => Some(Self::PromptInviteToken),
            ["v1", "wizard", "cancel"] => Some(Self::CancelWizard),
            ["v1", "admin", "pending"] => Some(Self::ShowPendingRequests),
            ["v1", "admin", "pending", "open", request_id] => Some(Self::OpenPendingRequest {
                request_id: parse_i64(request_id)?,
            }),
            ["v1", "admin", "users", "page", page] => Some(Self::ShowUsersPage {
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "user", "open", tg_user_id, page] => Some(Self::OpenUserCard {
                tg_user_id: parse_i64(tg_user_id)?,
                page: parse_i64(page)?.max(1),
            }),
            ["v1", "admin", "user", "view", tg_user_id] => Some(Self::ViewUserQr {
                tg_user_id: parse_i64(tg_user_id)?,
            }),
            ["v1", "admin", "user", "ban_confirm", tg_user_id, page] => {
                Some(Self::ConfirmUserBan {
                    tg_user_id: parse_i64(tg_user_id)?,
                    page: parse_i64(page)?.max(1),
                })
            }
            ["v1", "admin", "user", "ban_execute", tg_user_id, page] => {
                Some(Self::ExecuteUserBan {
                    tg_user_id: parse_i64(tg_user_id)?,
                    page: parse_i64(page)?.max(1),
                })
            }
            ["v1", "admin", "stats"] => Some(Self::ShowStats),
            ["v1", "admin", "service"] => Some(Self::ShowServicePanel),
            ["v1", "admin", "service", "run", action] => Some(Self::RunServiceAction {
                action: ServiceAction::parse(action)?,
            }),
            ["v1", "admin", "token"] => Some(Self::ShowTokenMenu),
            ["v1", "admin", "token", "create", auto_approve] => Some(Self::PromptTokenCreate {
                auto_approve: *auto_approve == "1",
            }),
            ["v1", "admin", "token", "list"] => Some(Self::ShowTokenList),
            ["v1", "admin", "token", "revoke"] => Some(Self::PromptTokenRevoke),
            ["v1", "admin", "create"] => Some(Self::PromptCreateUser),
            ["v1", "admin", "delete"] => Some(Self::PromptDeleteUser),
            ["v1", "admin", "delete", "confirm", tg_user_id] => Some(Self::ConfirmDeleteUser {
                tg_user_id: parse_i64(tg_user_id)?,
            }),
            ["v1", "admin", "delete", "execute", tg_user_id] => Some(Self::ExecuteDeleteUser {
                tg_user_id: parse_i64(tg_user_id)?,
            }),
            ["v1", "req", "approve", request_id] => Some(Self::ApproveRequest {
                request_id: parse_i64(request_id)?,
            }),
            ["v1", "req", "reject", request_id] => Some(Self::RejectRequest {
                request_id: parse_i64(request_id)?,
            }),
            _ => None,
        }
    }

    fn decode_legacy(data: &str) -> Option<Self> {
        if let Some(value) = data.strip_prefix("approve:") {
            return Some(Self::ApproveRequest {
                request_id: parse_i64(value)?,
            });
        }
        if let Some(value) = data.strip_prefix("reject:") {
            return Some(Self::RejectRequest {
                request_id: parse_i64(value)?,
            });
        }
        if let Some(value) = data.strip_prefix("users_page:") {
            return Some(Self::ShowUsersPage {
                page: parse_i64(value)?.max(1),
            });
        }
        if let Some(value) = data.strip_prefix("user_open:") {
            let (tg_user_id, page) = parse_pair(value)?;
            return Some(Self::OpenUserCard {
                tg_user_id,
                page: page.max(1),
            });
        }
        if let Some(value) = data.strip_prefix("user_view:") {
            let (tg_user_id, _) = parse_pair(value)?;
            return Some(Self::ViewUserQr { tg_user_id });
        }
        if let Some(value) = data.strip_prefix("user_ban:") {
            let (tg_user_id, page) = parse_pair(value)?;
            return Some(Self::ExecuteUserBan {
                tg_user_id,
                page: page.max(1),
            });
        }
        if let Some(value) = data.strip_prefix("delete_user:") {
            return Some(Self::ExecuteDeleteUser {
                tg_user_id: parse_i64(value)?,
            });
        }
        if let Some(value) = data.strip_prefix("service:") {
            return Some(Self::RunServiceAction {
                action: ServiceAction::parse(value).unwrap_or(ServiceAction::Status),
            });
        }
        None
    }
}

fn parse_i64(value: &str) -> Option<i64> {
    value.parse::<i64>().ok()
}

fn parse_pair(value: &str) -> Option<(i64, i64)> {
    let mut parts = value.split(':');
    let left = parse_i64(parts.next()?)?;
    let right = parse_i64(parts.next()?)?;
    Some((left, right))
}
