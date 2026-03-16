use crate::config::Config;
use crate::db::Db;
use crate::service::ServiceController;
use crate::telemt_cfg::TelemtConfig;
use std::sync::Arc;
use teloxide::types::Message;

#[derive(Clone, Debug)]
pub enum WizardState {
    AwaitingInviteToken,
    AdminCreateAwaitingTarget,
    AdminDeleteAwaitingTarget,
    AdminTokenCreateAwaitingParams { auto_approve: bool },
    AdminTokenRevokeAwaitingToken,
}

impl WizardState {
    fn encode(&self) -> String {
        match self {
            Self::AwaitingInviteToken => "awaiting_invite_token".to_string(),
            Self::AdminCreateAwaitingTarget => "admin_create_awaiting_target".to_string(),
            Self::AdminDeleteAwaitingTarget => "admin_delete_awaiting_target".to_string(),
            Self::AdminTokenCreateAwaitingParams { auto_approve } => {
                format!(
                    "admin_token_create:{}",
                    if *auto_approve { "auto" } else { "manual" }
                )
            }
            Self::AdminTokenRevokeAwaitingToken => "admin_token_revoke_awaiting_token".to_string(),
        }
    }

    fn decode(value: &str) -> Option<Self> {
        match value {
            "awaiting_invite_token" => Some(Self::AwaitingInviteToken),
            "admin_create_awaiting_target" => Some(Self::AdminCreateAwaitingTarget),
            "admin_delete_awaiting_target" => Some(Self::AdminDeleteAwaitingTarget),
            "admin_token_revoke_awaiting_token" => Some(Self::AdminTokenRevokeAwaitingToken),
            "admin_token_create:auto" => {
                Some(Self::AdminTokenCreateAwaitingParams { auto_approve: true })
            }
            "admin_token_create:manual" => Some(Self::AdminTokenCreateAwaitingParams {
                auto_approve: false,
            }),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct BotState {
    pub config: Arc<Config>,
    pub db: Arc<Db>,
    pub telemt_cfg: Arc<TelemtConfig>,
    pub service: ServiceController,
    pub bot_username: Option<String>,
}

pub fn telemt_username(tg_user_id: i64) -> String {
    format!("tg_{}", tg_user_id)
}

pub fn sender_user_id(msg: &Message) -> Option<i64> {
    msg.from.as_ref().map(|user| user.id.0 as i64)
}

pub fn sender_display_name(msg: &Message) -> Option<String> {
    msg.from.as_ref().map(|user| {
        let mut full_name = user.first_name.clone();
        if let Some(last_name) = user.last_name.as_deref()
            && !last_name.trim().is_empty()
        {
            full_name.push(' ');
            full_name.push_str(last_name);
        }
        full_name
    })
}

pub fn is_admin_message(msg: &Message, state: &BotState) -> bool {
    sender_user_id(msg).is_some_and(|user_id| state.config.is_admin(user_id))
}

pub async fn wizard_state(
    state: &BotState,
    user_id: i64,
) -> Result<Option<WizardState>, anyhow::Error> {
    let Some(state_key) = state.db.get_wizard_state(user_id).await? else {
        return Ok(None);
    };
    let Some(decoded) = WizardState::decode(&state_key) else {
        tracing::warn!(
            user_id = user_id,
            state_key = %state_key,
            "Не удалось декодировать сохранённое wizard-состояние"
        );
        state.db.clear_wizard_state(user_id).await?;
        return Ok(None);
    };
    Ok(Some(decoded))
}

pub async fn set_wizard_state(
    state: &BotState,
    user_id: i64,
    wizard_state: WizardState,
) -> Result<(), anyhow::Error> {
    state
        .db
        .set_wizard_state(user_id, &wizard_state.encode())
        .await?;
    Ok(())
}

pub async fn clear_wizard_state(state: &BotState, user_id: i64) -> Result<(), anyhow::Error> {
    state.db.clear_wizard_state(user_id).await?;
    Ok(())
}
