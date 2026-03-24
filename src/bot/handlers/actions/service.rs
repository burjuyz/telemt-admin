use crate::bot::handlers::callback_data::ServiceAction;
use crate::bot::handlers::state::BotState;

pub async fn execute_service_action(state: &BotState, action: ServiceAction) -> String {
    let result = match action {
        ServiceAction::Start => state.telemt_runtime.start().await,
        ServiceAction::Stop => state.telemt_runtime.stop().await,
        ServiceAction::Restart => state.telemt_runtime.restart().await,
        ServiceAction::Reload => state.telemt_runtime.reload().await,
        ServiceAction::Status => state.telemt_runtime.status().await,
    };

    if result.success {
        format!("{}: OK", action.as_str())
    } else {
        format!("{}: {}", action.as_str(), result.stderr)
    }
}
