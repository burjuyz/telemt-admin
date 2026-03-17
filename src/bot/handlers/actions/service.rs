use crate::bot::handlers::callback_data::ServiceAction;
use crate::bot::handlers::state::BotState;

pub async fn execute_service_action(state: &BotState, action: ServiceAction) -> String {
    let result = match action {
        ServiceAction::Start => state.service.start().await,
        ServiceAction::Stop => state.service.stop().await,
        ServiceAction::Restart => state.service.restart().await,
        ServiceAction::Reload => state.service.reload().await,
        ServiceAction::Status => state.service.status().await,
    };

    if result.success {
        format!("{}: OK", action.as_str())
    } else {
        format!("{}: {}", action.as_str(), result.stderr)
    }
}
