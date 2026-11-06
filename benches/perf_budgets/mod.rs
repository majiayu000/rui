//! The budgets `perf_gate` holds frames to, read from `benchmarks/perf_budgets/`.

use serde::Deserialize;
use std::collections::BTreeMap;

/// One `<case>.json` per case, so changes to different cases do not conflict, and
/// `shared.json`.
const BUDGETS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/benchmarks/perf_budgets");

/// `<case>.json`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseBudget {
    /// Most instructions per frame, in millions.
    pub instructions_m: f64,
    /// Most heap allocations per frame.
    pub allocations: f64,
}

/// `shared.json`.
#[derive(Debug, Deserialize)]
pub struct Budgets {
    /// Where and when the budgets were measured; read by people, not checked.
    #[allow(dead_code)]
    pub calibration: String,
    /// Each case's budget, from its own file.
    #[serde(skip)]
    pub cases: BTreeMap<String, CaseBudget>,
    /// Most the 100 000-row virtual list may cost relative to 1 000 rows.
    pub virtual_list_scaling_max: f64,
    /// Footprint before any renderer exists: what RUI's own data costs, the same on
    /// any machine.
    pub footprint_cpu_mb_max: f64,
    /// Footprint once the long list has been rendered. GPU driver memory is part of
    /// it, so this varies with the GPU: hosted CI VMs measure about 20 MB more.
    pub footprint_mb_max: f64,
    /// Most of the window one changed row may draw again (#175).
    pub one_row_damage_max: f64,
    /// Most the heap may grow over `CHURN_FRAMES` frames of `views.churn`, in KB.
    pub churn_growth_kb_max: f64,
    pub pixel_checksum: String,
}

/// `shared.json` with every `<case>.json` beside it.
pub fn load_budgets() -> Result<Budgets, String> {
    let read = |path: &std::path::Path| {
        std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))
    };
    let dir = std::path::Path::new(BUDGETS_DIR);
    let shared = dir.join("shared.json");
    let mut budgets: Budgets = serde_json::from_str(&read(&shared)?)
        .map_err(|err| format!("{}: {err}", shared.display()))?;
    let entries = std::fs::read_dir(dir).map_err(|err| format!("{BUDGETS_DIR}: {err}"))?;
    for entry in entries {
        let path = entry.map_err(|err| format!("{BUDGETS_DIR}: {err}"))?.path();
        let Some(case) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".json"))
        else {
            return Err(format!("{}: not a <case>.json budget", path.display()));
        };
        if case == "shared" {
            continue;
        }
        let budget = serde_json::from_str(&read(&path)?)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        budgets.cases.insert(case.to_string(), budget);
    }
    Ok(budgets)
}
