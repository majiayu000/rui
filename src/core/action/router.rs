use super::types::{ActionId, ActionOutcome};

pub trait ActionHandler {
    fn action_handler_name(&self) -> &str;
    fn action_handler_enabled(&self) -> bool {
        true
    }
    fn run_action(&mut self, action: &ActionId) -> ActionOutcome;
}

#[derive(Default)]
pub struct ActionRouter<'a> {
    focused_handlers: Vec<&'a mut dyn ActionHandler>,
    component_handlers: Vec<&'a mut dyn ActionHandler>,
    app_handlers: Vec<&'a mut dyn ActionHandler>,
}

impl<'a> ActionRouter<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn focused(mut self, handler: &'a mut dyn ActionHandler) -> Self {
        self.focused_handlers.push(handler);
        self
    }

    pub fn component(mut self, handler: &'a mut dyn ActionHandler) -> Self {
        self.component_handlers.push(handler);
        self
    }

    pub fn app(mut self, handler: &'a mut dyn ActionHandler) -> Self {
        self.app_handlers.push(handler);
        self
    }

    pub fn route_action(&mut self, action: &ActionId) -> ActionOutcome {
        for handler in self.focused_handlers.iter_mut() {
            if let Some(outcome) = dispatch_to_handler(*handler, action) {
                return outcome;
            }
        }

        for handler in self.component_handlers.iter_mut() {
            if let Some(outcome) = dispatch_to_handler(*handler, action) {
                return outcome;
            }
        }

        for handler in self.app_handlers.iter_mut() {
            if let Some(outcome) = dispatch_to_handler(*handler, action) {
                return outcome;
            }
        }

        ActionOutcome::Ignored
    }
}

fn dispatch_to_handler(
    handler: &mut dyn ActionHandler,
    action: &ActionId,
) -> Option<ActionOutcome> {
    if action.requires_enabled() && !handler.action_handler_enabled() {
        return None;
    }

    let outcome = handler.run_action(action);
    if outcome.is_handled() {
        Some(outcome)
    } else {
        None
    }
}
