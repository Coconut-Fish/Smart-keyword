export type LanguageMode = "chinese" | "english";

export type ResolutionSource =
  | "manual_rule"
  | "learned_preference"
  | "heuristic"
  | "manual_observation"
  | "none";

export interface FocusedApp {
  executable: string;
  processPath?: string | null;
  windowTitle: string;
  pid: number;
}

export interface EngineSettings {
  autoSwitchEnabled: boolean;
  learningEnabled: boolean;
  pollIntervalMs: number;
  learningConfidenceThreshold: number;
  minLearningSamples: number;
}

export interface ManualRule {
  executable: string;
  preferredLanguage: LanguageMode;
  note?: string | null;
  updatedAtEpoch: number;
}

export interface LearnedPreferenceView {
  executable: string;
  preferredLanguage?: LanguageMode | null;
  chineseScore: number;
  englishScore: number;
  confidence: number;
  lastObservedEpoch: number;
}

export interface DecisionSnapshot {
  targetLanguage?: LanguageMode | null;
  source: ResolutionSource;
  reason: string;
}

export interface ActivityEvent {
  timestampEpoch: number;
  app: string;
  summary: string;
  detail: string;
}

export interface PlatformSnapshot {
  name: string;
  supportsFocusTracking: boolean;
  supportsInputControl: boolean;
  notes: string;
}

export interface AppOverview {
  platform: PlatformSnapshot;
  settings: EngineSettings;
  activeApp?: FocusedApp | null;
  currentInputMode?: LanguageMode | null;
  manualRules: ManualRule[];
  learnedPreferences: LearnedPreferenceView[];
  lastDecision?: DecisionSnapshot | null;
  recentEvents: ActivityEvent[];
}
