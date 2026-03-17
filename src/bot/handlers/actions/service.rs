use crate::bot::handlers::callback_data::ServiceAction;
use crate::bot::handlers::state::BotState;

pub fn execute_service_action(state: &BotState, action: ServiceAction) -> String {
    let result = match action {
        ServiceAction::Start => state.service.start(),
        ServiceAction::Stop => state.service.stop(),
        ServiceAction::Restart => state.service.restart(),
        ServiceAction::Reload => state.service.reload(),
        ServiceAction::Status => state.service.status(),
    };

    if result.success {
        format!("{}: OK", action.as_str())
    } else {
        format!("{}: {}", action.as_str(), result.stderr)
    }
}
