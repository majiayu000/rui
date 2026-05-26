mod runtime_baseline_support;

use runtime_baseline_support::cases::runtime_cases;
use runtime_baseline_support::config::RuntimeBaseline;
use runtime_baseline_support::measure::run_case;
use runtime_baseline_support::report::print_report;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let baseline = RuntimeBaseline::load()?;
    let mut measurements = Vec::new();

    for case in runtime_cases() {
        measurements.push(run_case(&case)?);
    }

    print_report(&baseline, &measurements)?;
    Ok(())
}
