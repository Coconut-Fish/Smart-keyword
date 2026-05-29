import { invoke } from "@tauri-apps/api/core";
import { AppOverview, LanguageMode } from "./types";

export async function getOverview() {
  return invoke<AppOverview>("get_overview");
}

export async function upsertManualRule(input: {
  executable: string;
  preferredLanguage: LanguageMode;
  note?: string | null;
}) {
  return invoke<AppOverview>("upsert_manual_rule", {
    executable: input.executable,
    preferred_language: input.preferredLanguage,
    note: input.note ?? null,
  });
}

export async function deleteManualRule(executable: string) {
  return invoke<AppOverview>("delete_manual_rule", { executable });
}

export async function updateSettings(input: {
  autoSwitchEnabled: boolean;
  learningEnabled: boolean;
}) {
  return invoke<AppOverview>("update_settings", {
    auto_switch_enabled: input.autoSwitchEnabled,
    learning_enabled: input.learningEnabled,
  });
}

export async function learnCurrentPreference() {
  return invoke<AppOverview>("learn_current_preference");
}
