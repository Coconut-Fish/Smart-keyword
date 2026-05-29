use crate::models::{FocusedApp, LanguageMode};

use super::{PlatformCapabilities, PlatformController};

#[derive(Default)]
pub struct UnsupportedPlatformController;

impl PlatformController for UnsupportedPlatformController {
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            name: std::env::consts::OS,
            supports_focus_tracking: false,
            supports_input_control: false,
            notes: "The current build ships with a Windows input-controller implementation. macOS and Linux extension points are ready, but their native adapters still need to be implemented.",
        }
    }

    fn active_application(&self) -> Result<Option<FocusedApp>, String> {
        Ok(None)
    }

    fn current_input_mode(&self, _app: &FocusedApp) -> Result<Option<LanguageMode>, String> {
        Ok(None)
    }

    fn set_input_mode(&self, _app: &FocusedApp, _mode: LanguageMode) -> Result<bool, String> {
        Ok(false)
    }
}
