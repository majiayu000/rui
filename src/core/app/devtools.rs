use super::AppContext;

impl AppContext {
    /// Show or hide the profiling overlay in every window. `RUI_DEVTOOLS=1`
    /// starts with it shown. See `docs/devtools.md`.
    pub fn toggle_profiler(&mut self) {
        self.devtools.profiler = !self.devtools.profiler;
        self.request_redraw();
    }

    pub fn profiler_enabled(&self) -> bool {
        self.devtools.profiler
    }

    /// Show or hide the damage and replay visualizer in every window.
    /// `RUI_DEVTOOLS=damage` starts with it shown. See `docs/devtools.md`.
    pub fn toggle_damage_visualizer(&mut self) {
        self.devtools.damage = !self.devtools.damage;
        self.request_redraw();
    }

    pub fn damage_visualizer_enabled(&self) -> bool {
        self.devtools.damage
    }

    /// Turn the inspector on or off in every window. While it is on, pointer
    /// presses and moves pick elements instead of reaching the app.
    /// `RUI_DEVTOOLS=inspector` starts with it on. See `docs/devtools.md`.
    pub fn toggle_inspector(&mut self) {
        self.devtools.inspector = !self.devtools.inspector;
        self.request_redraw();
    }

    pub fn inspector_enabled(&self) -> bool {
        self.devtools.inspector
    }

}
