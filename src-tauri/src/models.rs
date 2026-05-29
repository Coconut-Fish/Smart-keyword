use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageMode {
    Chinese,
    English,
}

impl LanguageMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Chinese => "中文",
            Self::English => "English",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionSource {
    ManualRule,
    LearnedPreference,
    Heuristic,
    ManualObservation,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusedApp {
    pub executable: String,
    pub process_path: Option<String>,
    pub window_title: String,
    pub pid: u32,
}

impl FocusedApp {
    pub fn app_key(&self) -> String {
        self.executable.trim().to_ascii_lowercase()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualRule {
    pub executable: String,
    pub preferred_language: LanguageMode,
    pub note: Option<String>,
    pub updated_at_epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LearnedAppPreference {
    pub executable: String,
    pub chinese_score: u32,
    pub english_score: u32,
    pub last_observed_epoch: u64,
}

impl LearnedAppPreference {
    pub fn record(&mut self, language: LanguageMode, weight: u32, timestamp: u64) {
        self.last_observed_epoch = timestamp;
        match language {
            LanguageMode::Chinese => {
                self.chinese_score = self.chinese_score.saturating_add(weight);
            }
            LanguageMode::English => {
                self.english_score = self.english_score.saturating_add(weight);
            }
        }
    }

    pub fn total(&self) -> u32 {
        self.chinese_score.saturating_add(self.english_score)
    }

    pub fn confidence(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            self.chinese_score.max(self.english_score) as f32 / total as f32
        }
    }

    pub fn preferred_language(&self) -> Option<LanguageMode> {
        if self.chinese_score == self.english_score {
            None
        } else if self.chinese_score > self.english_score {
            Some(LanguageMode::Chinese)
        } else {
            Some(LanguageMode::English)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineSettings {
    pub auto_switch_enabled: bool,
    pub learning_enabled: bool,
    pub poll_interval_ms: u64,
    pub learning_confidence_threshold: f32,
    pub min_learning_samples: u32,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            auto_switch_enabled: true,
            learning_enabled: true,
            poll_interval_ms: 900,
            learning_confidence_threshold: 0.72,
            min_learning_samples: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PersistedState {
    pub settings: EngineSettings,
    pub manual_rules: Vec<ManualRule>,
    pub learned_preferences: Vec<LearnedAppPreference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionSnapshot {
    pub target_language: Option<LanguageMode>,
    pub source: ResolutionSource,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub timestamp_epoch: u64,
    pub app: String,
    pub summary: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnedPreferenceView {
    pub executable: String,
    pub preferred_language: Option<LanguageMode>,
    pub chinese_score: u32,
    pub english_score: u32,
    pub confidence: f32,
    pub last_observed_epoch: u64,
}

impl From<&LearnedAppPreference> for LearnedPreferenceView {
    fn from(value: &LearnedAppPreference) -> Self {
        Self {
            executable: value.executable.clone(),
            preferred_language: value.preferred_language(),
            chinese_score: value.chinese_score,
            english_score: value.english_score,
            confidence: value.confidence(),
            last_observed_epoch: value.last_observed_epoch,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformSnapshot {
    pub name: String,
    pub supports_focus_tracking: bool,
    pub supports_input_control: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppOverview {
    pub platform: PlatformSnapshot,
    pub settings: EngineSettings,
    pub active_app: Option<FocusedApp>,
    pub current_input_mode: Option<LanguageMode>,
    pub manual_rules: Vec<ManualRule>,
    pub learned_preferences: Vec<LearnedPreferenceView>,
    pub last_decision: Option<DecisionSnapshot>,
    pub recent_events: Vec<ActivityEvent>,
}
