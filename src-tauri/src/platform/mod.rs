use crate::models::{FocusedApp, LanguageMode};

#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_os = "windows"))]
mod unsupported;

pub struct PlatformCapabilities {
    pub name: &'static str,
    pub supports_focus_tracking: bool,
    pub supports_input_control: bool,
    pub notes: &'static str,
}

pub trait PlatformController: Send + Sync {
    fn capabilities(&self) -> PlatformCapabilities;
    fn active_application(&self) -> Result<Option<FocusedApp>, String>;
    fn current_input_mode(&self, app: &FocusedApp) -> Result<Option<LanguageMode>, String>;
    fn set_input_mode(&self, app: &FocusedApp, mode: LanguageMode) -> Result<bool, String>;
}

#[cfg(target_os = "windows")]
pub fn create_controller() -> Box<dyn PlatformController> {
    Box::new(windows::WindowsPlatformController)
}

#[cfg(not(target_os = "windows"))]
pub fn create_controller() -> Box<dyn PlatformController> {
    Box::new(unsupported::UnsupportedPlatformController::default())
}
