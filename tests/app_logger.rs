use log::{Log, Metadata, Record};
use rui::App;

struct ApplicationLogger;

impl Log for ApplicationLogger {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        false
    }

    fn log(&self, _: &Record<'_>) {}

    fn flush(&self) {}
}

#[test]
fn app_creation_leaves_logger_registration_to_the_application() {
    let _app = App::new();
    log::set_logger(&ApplicationLogger).expect("the application owns logger registration");
}
