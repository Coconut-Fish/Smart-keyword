import { FormEvent, startTransition, useEffect, useEffectEvent, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  deleteManualRule,
  getOverview,
  learnCurrentPreference,
  updateSettings,
  upsertManualRule,
} from "./api";
import {
  AppOverview,
  LanguageMode,
  LearnedPreferenceView,
  ManualRule,
  ResolutionSource,
} from "./types";
import "./App.css";

const languageOptions: Array<{ label: string; value: LanguageMode }> = [
  { label: "中文", value: "chinese" },
  { label: "English", value: "english" },
];

function App() {
  const [overview, setOverview] = useState<AppOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [ruleExecutable, setRuleExecutable] = useState("");
  const [ruleLanguage, setRuleLanguage] = useState<LanguageMode>("chinese");
  const [ruleNote, setRuleNote] = useState("");

  const applyOverview = useEffectEvent((next: AppOverview) => {
    startTransition(() => {
      setOverview(next);
    });
  });

  const refresh = useEffectEvent(async () => {
    try {
      const next = await getOverview();
      applyOverview(next);
      setError(null);
    } catch (reason) {
      setError(asMessage(reason));
    }
  });

  useEffect(() => {
    void refresh();

    const unlisten = listen<AppOverview>("smart-keyword://overview-updated", (event) => {
      applyOverview(event.payload);
    });

    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [applyOverview, refresh]);

  const sortedRules = useMemo(
    () =>
      [...(overview?.manualRules ?? [])].sort((left, right) =>
        left.executable.localeCompare(right.executable),
      ),
    [overview],
  );

  async function handleSettingsChange(
    next: Pick<AppOverview["settings"], "autoSwitchEnabled" | "learningEnabled">,
  ) {
    if (!overview) {
      return;
    }

    setBusyAction("settings");
    try {
      const nextOverview = await updateSettings(next);
      applyOverview(nextOverview);
      setError(null);
    } catch (reason) {
      setError(asMessage(reason));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleSubmitRule(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusyAction("save-rule");

    try {
      const nextOverview = await upsertManualRule({
        executable: ruleExecutable,
        preferredLanguage: ruleLanguage,
        note: ruleNote.trim() || null,
      });
      applyOverview(nextOverview);
      setRuleNote("");
      setError(null);
    } catch (reason) {
      setError(asMessage(reason));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleDeleteRule(rule: ManualRule) {
    setBusyAction(`delete-${rule.executable}`);

    try {
      const nextOverview = await deleteManualRule(rule.executable);
      applyOverview(nextOverview);
      setError(null);
    } catch (reason) {
      setError(asMessage(reason));
    } finally {
      setBusyAction(null);
    }
  }

  async function handlePromotePreference(entry: LearnedPreferenceView) {
    if (!entry.preferredLanguage) {
      return;
    }

    setBusyAction(`promote-${entry.executable}`);

    try {
      const nextOverview = await upsertManualRule({
        executable: entry.executable,
        preferredLanguage: entry.preferredLanguage,
        note: "由学习偏好提升为手动规则",
      });
      applyOverview(nextOverview);
      setError(null);
    } catch (reason) {
      setError(asMessage(reason));
    } finally {
      setBusyAction(null);
    }
  }

  async function handleLearnCurrent() {
    setBusyAction("learn-current");

    try {
      const nextOverview = await learnCurrentPreference();
      applyOverview(nextOverview);
      setError(null);
    } catch (reason) {
      setError(asMessage(reason));
    } finally {
      setBusyAction(null);
    }
  }

  const activeExecutable = overview?.activeApp?.executable ?? "未检测到";
  const currentMode = labelForMode(overview?.currentInputMode);
  const sourceLabel = labelForSource(overview?.lastDecision?.source);

  return (
    <main className="app-shell">
      <section className="hero">
        <div>
          <p className="eyebrow">Smart Keyword</p>
          <h1>切换应用时，让输入法自己回到对的语言。</h1>
          <p className="hero-copy">
            规则优先、学习补位，只在应用焦点切换时触发，不干扰你在应用内部手动切换中英文。
          </p>
        </div>

        <div className="hero-status">
          <StatusPill
            label="自动切换"
            value={overview?.settings.autoSwitchEnabled ? "开启" : "关闭"}
            accent={overview?.settings.autoSwitchEnabled ? "ok" : "soft"}
          />
          <StatusPill
            label="自学习"
            value={overview?.settings.learningEnabled ? "开启" : "关闭"}
            accent={overview?.settings.learningEnabled ? "ok" : "soft"}
          />
          <StatusPill
            label="当前平台"
            value={overview?.platform.name ?? "loading"}
            accent={overview?.platform.supportsInputControl ? "ok" : "warn"}
          />
        </div>
      </section>

      {error ? <section className="banner error">{error}</section> : null}

      <section className="dashboard-grid">
        <article className="panel panel-spotlight">
          <div className="panel-heading">
            <div>
              <p className="panel-kicker">实时状态</p>
              <h2>当前焦点与决策</h2>
            </div>
            <button className="ghost-button" type="button" onClick={() => void refresh()}>
              刷新
            </button>
          </div>

          <div className="metrics">
            <MetricCard label="前台应用" value={activeExecutable} detail={overview?.activeApp?.windowTitle} />
            <MetricCard label="当前输入法" value={currentMode} detail={overview?.activeApp?.processPath ?? "等待检测"} />
            <MetricCard
              label="决策来源"
              value={sourceLabel}
              detail={overview?.lastDecision?.reason ?? "还没有产生决策"}
            />
          </div>

          <div className="action-row">
            <button
              className="primary-button"
              type="button"
              disabled={!overview?.activeApp}
              onClick={() => setRuleExecutable(overview?.activeApp?.executable ?? "")}
            >
              用当前应用填充规则
            </button>
            <button
              className="secondary-button"
              type="button"
              disabled={busyAction === "learn-current"}
              onClick={() => void handleLearnCurrent()}
            >
              将当前状态记为学习样本
            </button>
          </div>
        </article>

        <article className="panel">
          <div className="panel-heading">
            <div>
              <p className="panel-kicker">行为开关</p>
              <h2>切换策略</h2>
            </div>
          </div>

          <label className="toggle-card">
            <div>
              <strong>只在应用切换时自动调整</strong>
              <p>进入新应用时做一次判断，不追着你在当前应用里的手动切换跑。</p>
            </div>
            <input
              type="checkbox"
              checked={overview?.settings.autoSwitchEnabled ?? false}
              disabled={!overview || busyAction === "settings"}
              onChange={(event) =>
                void handleSettingsChange({
                  autoSwitchEnabled: event.currentTarget.checked,
                  learningEnabled: overview?.settings.learningEnabled ?? true,
                })
              }
            />
          </label>

          <label className="toggle-card">
            <div>
              <strong>允许后台自学习</strong>
              <p>对没有手动规则的应用，根据你最终稳定使用的语言积累偏好。</p>
            </div>
            <input
              type="checkbox"
              checked={overview?.settings.learningEnabled ?? false}
              disabled={!overview || busyAction === "settings"}
              onChange={(event) =>
                void handleSettingsChange({
                  autoSwitchEnabled: overview?.settings.autoSwitchEnabled ?? true,
                  learningEnabled: event.currentTarget.checked,
                })
              }
            />
          </label>

          <div className="hint-card">
            <h3>当前实现说明</h3>
            <p>{overview?.platform.notes ?? "正在读取平台能力..."}</p>
          </div>
        </article>
      </section>

      <section className="workspace-grid">
        <article className="panel">
          <div className="panel-heading">
            <div>
              <p className="panel-kicker">规则优先</p>
              <h2>手动规则</h2>
            </div>
          </div>

          <form className="rule-form" onSubmit={handleSubmitRule}>
            <label>
              应用可执行文件
              <input
                value={ruleExecutable}
                onChange={(event) => setRuleExecutable(event.currentTarget.value)}
                placeholder="例如 WeChat.exe / Code.exe"
              />
            </label>

            <label>
              进入该应用时使用
              <select
                value={ruleLanguage}
                onChange={(event) => setRuleLanguage(event.currentTarget.value as LanguageMode)}
              >
                {languageOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label>
              备注
              <input
                value={ruleNote}
                onChange={(event) => setRuleNote(event.currentTarget.value)}
                placeholder="例如 聊天场景以中文为主"
              />
            </label>

            <button className="primary-button" type="submit" disabled={busyAction === "save-rule"}>
              保存规则
            </button>
          </form>

          <div className="list-block">
            {sortedRules.length === 0 ? (
              <EmptyState
                title="还没有手动规则"
                copy="先给微信、Code、终端这类高频应用设规则，效果会立刻稳定下来。"
              />
            ) : (
              sortedRules.map((rule) => (
                <RuleRow
                  key={rule.executable}
                  title={rule.executable}
                  badge={labelForMode(rule.preferredLanguage)}
                  meta={rule.note || formatTime(rule.updatedAtEpoch)}
                  actionLabel="删除"
                  disabled={busyAction === `delete-${rule.executable}`}
                  onAction={() => void handleDeleteRule(rule)}
                />
              ))
            )}
          </div>
        </article>

        <article className="panel">
          <div className="panel-heading">
            <div>
              <p className="panel-kicker">学习补位</p>
              <h2>学习偏好</h2>
            </div>
          </div>

          <div className="list-block">
            {(overview?.learnedPreferences ?? []).length === 0 ? (
              <EmptyState
                title="还没有形成偏好"
                copy="开始切换应用并正常使用一段时间，系统会积累没有手动规则应用的中英文倾向。"
              />
            ) : (
              overview?.learnedPreferences.map((entry) => (
                <RuleRow
                  key={entry.executable}
                  title={entry.executable}
                  badge={labelForMode(entry.preferredLanguage)}
                  meta={`置信度 ${Math.round(entry.confidence * 100)}% · 中文 ${entry.chineseScore} / 英文 ${entry.englishScore}`}
                  actionLabel={entry.preferredLanguage ? "设为手动规则" : "继续观察"}
                  disabled={!entry.preferredLanguage || busyAction === `promote-${entry.executable}`}
                  onAction={() => void handlePromotePreference(entry)}
                />
              ))
            )}
          </div>
        </article>

        <article className="panel panel-log">
          <div className="panel-heading">
            <div>
              <p className="panel-kicker">最近活动</p>
              <h2>观察日志</h2>
            </div>
          </div>

          <div className="timeline">
            {(overview?.recentEvents ?? []).length === 0 ? (
              <EmptyState title="日志暂时为空" copy="切换几个应用后，这里会显示规则命中、自动调整和学习记录。" />
            ) : (
              overview?.recentEvents.map((entry) => (
                <div className="timeline-item" key={`${entry.timestampEpoch}-${entry.app}-${entry.summary}`}>
                  <div className="timeline-dot" />
                  <div>
                    <div className="timeline-head">
                      <strong>{entry.summary}</strong>
                      <span>{formatTime(entry.timestampEpoch)}</span>
                    </div>
                    <p>{entry.app}</p>
                    <small>{entry.detail}</small>
                  </div>
                </div>
              ))
            )}
          </div>
        </article>
      </section>
    </main>
  );
}

function MetricCard({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return (
    <div className="metric-card">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail || "暂无更多信息"}</small>
    </div>
  );
}

function StatusPill({
  label,
  value,
  accent,
}: {
  label: string;
  value: string;
  accent: "ok" | "warn" | "soft";
}) {
  return (
    <div className={`status-pill status-pill-${accent}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function RuleRow({
  title,
  badge,
  meta,
  actionLabel,
  disabled,
  onAction,
}: {
  title: string;
  badge: string;
  meta: string;
  actionLabel: string;
  disabled?: boolean;
  onAction: () => void;
}) {
  return (
    <div className="rule-row">
      <div>
        <div className="rule-title">
          <strong>{title}</strong>
          <span>{badge}</span>
        </div>
        <p>{meta}</p>
      </div>
      <button className="ghost-button" type="button" disabled={disabled} onClick={onAction}>
        {actionLabel}
      </button>
    </div>
  );
}

function EmptyState({ title, copy }: { title: string; copy: string }) {
  return (
    <div className="empty-state">
      <strong>{title}</strong>
      <p>{copy}</p>
    </div>
  );
}

function labelForMode(mode?: LanguageMode | null) {
  if (mode === "chinese") {
    return "中文";
  }

  if (mode === "english") {
    return "English";
  }

  return "未知";
}

function labelForSource(source?: ResolutionSource | null) {
  switch (source) {
    case "manual_rule":
      return "手动规则";
    case "learned_preference":
      return "学习偏好";
    case "heuristic":
      return "启发式判断";
    case "manual_observation":
      return "手动学习";
    default:
      return "暂无";
  }
}

function formatTime(epoch?: number) {
  if (!epoch) {
    return "刚刚";
  }

  return new Date(epoch * 1000).toLocaleString("zh-CN", {
    hour12: false,
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function asMessage(reason: unknown) {
  if (reason instanceof Error) {
    return reason.message;
  }

  if (typeof reason === "string") {
    return reason;
  }

  return "发生了一个未识别的错误。";
}

export default App;
