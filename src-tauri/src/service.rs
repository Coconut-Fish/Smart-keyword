use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::models::{
    ActivityEvent, AppOverview, DecisionSnapshot, EngineSettings, FocusedApp, LanguageMode,
    LearnedAppPreference, LearnedPreferenceView, ManualRule, PersistedState, PlatformSnapshot,
    ResolutionSource,
};
use crate::platform::{create_controller, PlatformController};
use crate::storage::Storage;

const MAX_RECENT_EVENTS: usize = 16;
const SESSION_GUARD_MS: u64 = 1400;
const MIN_LEARNING_SESSION_MS: u64 = 4000;

struct AppSession {
    app: FocusedApp,
    entered_at: Instant,
    initial_mode: Option<LanguageMode>,
    auto_applied_mode: Option<LanguageMode>,
    latest_observed_mode: Option<LanguageMode>,
    settled_mode: Option<LanguageMode>,
    switch_guard_until: Option<Instant>,
}

pub struct SmartSwitcherService {
    storage: Storage,
    controller: Box<dyn PlatformController>,
    settings: EngineSettings,
    manual_rules: BTreeMap<String, ManualRule>,
    learned_preferences: HashMap<String, LearnedAppPreference>,
    active_app: Option<FocusedApp>,
    current_input_mode: Option<LanguageMode>,
    last_decision: Option<DecisionSnapshot>,
    recent_events: VecDeque<ActivityEvent>,
    current_session: Option<AppSession>,
}

impl SmartSwitcherService {
    pub fn new() -> Self {
        Self {
            storage: Storage::new().unwrap_or_else(|_| {
                let fallback_path = std::env::current_dir()
                    .unwrap_or_default()
                    .join(".smart-keyboard-fallback")
                    .join("state.json");
                Storage::from_path(fallback_path).expect("unable to initialize fallback storage")
            }),
            controller: create_controller(),
            settings: EngineSettings::default(),
            manual_rules: BTreeMap::new(),
            learned_preferences: HashMap::new(),
            active_app: None,
            current_input_mode: None,
            last_decision: None,
            recent_events: VecDeque::new(),
            current_session: None,
        }
    }

    pub fn initialize(&mut self) -> Result<(), String> {
        let persisted = self.storage.load()?;
        self.settings = persisted.settings;
        self.manual_rules = persisted
            .manual_rules
            .into_iter()
            .map(|rule| (normalize_key(&rule.executable), rule))
            .collect();
        self.learned_preferences = persisted
            .learned_preferences
            .into_iter()
            .map(|entry| (normalize_key(&entry.executable), entry))
            .collect();

        let capabilities = self.controller.capabilities();
        self.push_event(
            "system".to_string(),
            "后台服务已启动".to_string(),
            capabilities.notes.to_string(),
        );

        Ok(())
    }

    pub fn poll_once(&mut self) -> Result<(), String> {
        let active = self.controller.active_application()?;
        let current_mode = if let Some(app) = active.as_ref() {
            self.controller.current_input_mode(app)?
        } else {
            None
        };

        if let Some(session) = self.current_session.as_mut() {
            if let Some(app) = active.as_ref() {
                if session.app.app_key() == app.app_key() {
                    if let Some(until) = session.switch_guard_until {
                        if Instant::now() >= until {
                            session.switch_guard_until = None;
                        }
                    }

                    if current_mode.is_some() {
                        session.latest_observed_mode = current_mode;

                        if session.switch_guard_until.is_none() {
                            session.settled_mode = current_mode;
                        }
                    }
                }
            }
        }

        let app_changed = match (&self.active_app, &active) {
            (Some(previous), Some(next)) => previous.app_key() != next.app_key(),
            (None, Some(_)) | (Some(_), None) => true,
            (None, None) => false,
        };

        if app_changed {
            self.finalize_previous_session();
        }

        self.active_app = active.clone();
        self.current_input_mode = current_mode;

        if app_changed {
            if let Some(app) = active {
                self.handle_app_focus(app)?;
            } else {
                self.last_decision = Some(DecisionSnapshot {
                    target_language: None,
                    source: ResolutionSource::None,
                    reason: "当前没有可识别的前台应用。".to_string(),
                });
            }
        }

        Ok(())
    }

    pub fn overview(&self) -> AppOverview {
        let capabilities = self.controller.capabilities();
        let mut learned_preferences = self
            .learned_preferences
            .values()
            .map(LearnedPreferenceView::from)
            .collect::<Vec<_>>();

        learned_preferences.sort_by(|left, right| {
            right
                .confidence
                .partial_cmp(&left.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.last_observed_epoch.cmp(&left.last_observed_epoch))
        });

        AppOverview {
            platform: PlatformSnapshot {
                name: capabilities.name.to_string(),
                supports_focus_tracking: capabilities.supports_focus_tracking,
                supports_input_control: capabilities.supports_input_control,
                notes: capabilities.notes.to_string(),
            },
            settings: self.settings.clone(),
            active_app: self.active_app.clone(),
            current_input_mode: self.current_input_mode,
            manual_rules: self.manual_rules.values().cloned().collect(),
            learned_preferences,
            last_decision: self.last_decision.clone(),
            recent_events: self.recent_events.iter().cloned().collect(),
        }
    }

    pub fn upsert_manual_rule(
        &mut self,
        executable: String,
        preferred_language: LanguageMode,
        note: Option<String>,
    ) -> Result<(), String> {
        let executable = executable.trim().to_string();
        if executable.is_empty() {
            return Err("应用名不能为空。".to_string());
        }

        let rule = ManualRule {
            executable: executable.clone(),
            preferred_language,
            note,
            updated_at_epoch: now_epoch(),
        };
        self.manual_rules
            .insert(normalize_key(&executable), rule.clone());
        self.persist()?;
        self.push_event(
            executable.clone(),
            "已保存手动规则".to_string(),
            format!("切换到该应用时优先使用 {}。", preferred_language.label()),
        );

        Ok(())
    }

    pub fn delete_manual_rule(&mut self, executable: &str) -> Result<(), String> {
        let key = normalize_key(executable);
        self.manual_rules.remove(&key);
        self.persist()?;
        self.push_event(
            executable.to_string(),
            "已删除手动规则".to_string(),
            "之后将回退到学习偏好或启发式判断。".to_string(),
        );
        Ok(())
    }

    pub fn update_settings(
        &mut self,
        auto_switch_enabled: bool,
        learning_enabled: bool,
    ) -> Result<(), String> {
        self.settings.auto_switch_enabled = auto_switch_enabled;
        self.settings.learning_enabled = learning_enabled;
        self.persist()?;
        self.push_event(
            "settings".to_string(),
            "设置已更新".to_string(),
            format!(
                "自动切换：{}，自学习：{}。",
                yes_no(auto_switch_enabled),
                yes_no(learning_enabled)
            ),
        );
        Ok(())
    }

    pub fn learn_current_preference(&mut self) -> Result<(), String> {
        let app = self
            .active_app
            .clone()
            .ok_or_else(|| "当前没有前台应用，无法学习。".to_string())?;
        let mode = self
            .current_input_mode
            .ok_or_else(|| "当前无法读取输入法状态，无法学习。".to_string())?;

        self.record_learning(&app.app_key(), &app.executable, mode, 3)?;
        self.last_decision = Some(DecisionSnapshot {
            target_language: Some(mode),
            source: ResolutionSource::ManualObservation,
            reason: "已根据你当前在该应用中的输入法状态补充一条学习样本。".to_string(),
        });
        self.push_event(
            app.executable.clone(),
            "已手动强化学习样本".to_string(),
            format!("记录为 {} 偏好。", mode.label()),
        );
        Ok(())
    }

    fn handle_app_focus(&mut self, app: FocusedApp) -> Result<(), String> {
        let current_mode = self.controller.current_input_mode(&app)?;
        self.current_input_mode = current_mode;

        let decision = self.resolve_target_language(&app);
        let mut auto_applied_mode = None;
        let mut guard_until = None;

        if self.settings.auto_switch_enabled
            && self.controller.capabilities().supports_input_control
            && decision.target_language.is_some()
            && current_mode != decision.target_language
        {
            if let Some(target) = decision.target_language {
                let switch_succeeded = self.controller.set_input_mode(&app, target)?;
                let refreshed_mode = self.controller.current_input_mode(&app)?;
                self.current_input_mode = refreshed_mode;

                if switch_succeeded && refreshed_mode == Some(target) {
                    auto_applied_mode = Some(target);
                    guard_until = Some(Instant::now() + Duration::from_millis(SESSION_GUARD_MS));
                    self.push_event(
                        app.executable.clone(),
                        "切换焦点时自动调整输入法".to_string(),
                        format!("{} -> {}", source_label(decision.source), target.label()),
                    );
                } else {
                    self.push_event(
                        app.executable.clone(),
                        "尝试自动调整输入法但未确认成功".to_string(),
                        format!(
                            "目标为 {}，当前检测结果为 {}。",
                            target.label(),
                            refreshed_mode.map(LanguageMode::label).unwrap_or("未知")
                        ),
                    );
                }
            }
        } else {
            self.push_event(
                app.executable.clone(),
                "检测到应用切换".to_string(),
                decision.reason.clone(),
            );
        }

        self.last_decision = Some(decision.clone());
        self.current_session = Some(AppSession {
            app,
            entered_at: Instant::now(),
            initial_mode: current_mode,
            auto_applied_mode,
            latest_observed_mode: self.current_input_mode.or(current_mode),
            settled_mode: auto_applied_mode
                .or(self.current_input_mode)
                .or(current_mode),
            switch_guard_until: guard_until,
        });
        Ok(())
    }

    fn finalize_previous_session(&mut self) {
        let Some(session) = self.current_session.take() else {
            return;
        };

        if !self.settings.learning_enabled {
            return;
        }

        if self.manual_rules.contains_key(&session.app.app_key()) {
            return;
        }

        if session.entered_at.elapsed().as_millis() < MIN_LEARNING_SESSION_MS as u128 {
            return;
        }

        let Some(final_mode) = session
            .latest_observed_mode
            .or(session.settled_mode)
            .or(session.initial_mode)
        else {
            return;
        };

        let weight = match session.auto_applied_mode {
            Some(auto_mode) if auto_mode != final_mode => 3,
            Some(_) => 1,
            None => 2,
        };

        if self
            .record_learning(
                &session.app.app_key(),
                &session.app.executable,
                final_mode,
                weight,
            )
            .is_ok()
        {
            self.push_event(
                session.app.executable.clone(),
                "已更新学习偏好".to_string(),
                format!(
                    "本次会话最终稳定在 {}，已写入学习模型。",
                    final_mode.label()
                ),
            );
        }
    }

    fn resolve_target_language(&self, app: &FocusedApp) -> DecisionSnapshot {
        if let Some(rule) = self.manual_rules.get(&app.app_key()) {
            return DecisionSnapshot {
                target_language: Some(rule.preferred_language),
                source: ResolutionSource::ManualRule,
                reason: format!(
                    "命中手动规则：{} -> {}。",
                    rule.executable,
                    rule.preferred_language.label()
                ),
            };
        }

        if let Some(preference) = self.learned_preferences.get(&app.app_key()) {
            let confidence = preference.confidence();
            if preference.total() >= self.settings.min_learning_samples
                && confidence >= self.settings.learning_confidence_threshold
            {
                if let Some(language) = preference.preferred_language() {
                    return DecisionSnapshot {
                        target_language: Some(language),
                        source: ResolutionSource::LearnedPreference,
                        reason: format!(
                            "命中学习偏好：置信度 {:.0}%（{} / {}）。",
                            confidence * 100.0,
                            preference.chinese_score,
                            preference.english_score
                        ),
                    };
                }
            }
        }

        if let Some(language) = heuristic_language_for_app(app) {
            return DecisionSnapshot {
                target_language: Some(language),
                source: ResolutionSource::Heuristic,
                reason: format!(
                    "根据应用类型启发式判断：{} 更可能需要 {}。",
                    app.executable,
                    language.label()
                ),
            };
        }

        DecisionSnapshot {
            target_language: None,
            source: ResolutionSource::None,
            reason: "当前没有命中规则，也还没有足够的学习样本。".to_string(),
        }
    }

    fn record_learning(
        &mut self,
        key: &str,
        executable: &str,
        language: LanguageMode,
        weight: u32,
    ) -> Result<(), String> {
        let entry = self
            .learned_preferences
            .entry(key.to_string())
            .or_insert_with(|| LearnedAppPreference {
                executable: executable.to_string(),
                ..LearnedAppPreference::default()
            });

        entry.record(language, weight, now_epoch());
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        self.storage.save(&PersistedState {
            settings: self.settings.clone(),
            manual_rules: self.manual_rules.values().cloned().collect(),
            learned_preferences: self.learned_preferences.values().cloned().collect(),
        })
    }

    fn push_event(&mut self, app: String, summary: String, detail: String) {
        self.recent_events.push_front(ActivityEvent {
            timestamp_epoch: now_epoch(),
            app,
            summary,
            detail,
        });

        while self.recent_events.len() > MAX_RECENT_EVENTS {
            self.recent_events.pop_back();
        }
    }
}

fn heuristic_language_for_app(app: &FocusedApp) -> Option<LanguageMode> {
    let key = app.app_key();
    let title = app.window_title.to_ascii_lowercase();

    if contains_any(
        &key,
        &[
            "wechat", "weixin", "qq", "tim", "dingtalk", "feishu", "lark", "wecom", "wxwork",
            "music", "notes", "notion",
        ],
    ) || contains_any(&title, &["微信", "聊天", "消息", "备注", "文档"])
    {
        return Some(LanguageMode::Chinese);
    }

    if contains_any(
        &key,
        &[
            "code",
            "cursor",
            "idea",
            "pycharm",
            "goland",
            "clion",
            "webstorm",
            "rider",
            "datagrip",
            "terminal",
            "powershell",
            "cmd",
            "alacritty",
            "wezterm",
            "kitty",
            "bash",
            "zsh",
            "git",
            "devenv",
            "notepad++",
        ],
    ) || contains_any(
        &title,
        &["terminal", "shell", "code", "github", "commit", "debug"],
    ) {
        return Some(LanguageMode::English);
    }

    None
}

fn contains_any(input: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| input.contains(keyword))
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn source_label(source: ResolutionSource) -> &'static str {
    match source {
        ResolutionSource::ManualRule => "手动规则",
        ResolutionSource::LearnedPreference => "学习偏好",
        ResolutionSource::Heuristic => "启发式判断",
        ResolutionSource::ManualObservation => "手动学习",
        ResolutionSource::None => "无决策",
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "开启"
    } else {
        "关闭"
    }
}
