import { useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, type Permissions, type Settings, type StateEvent, type UiState } from "./ipc";
import "./App.css";

const surface = new URLSearchParams(location.search).get("surface") ?? "command";
document.documentElement.dataset.surface = surface;
const isMac = /Mac/.test(navigator.platform);
const primary = isMac ? "⌘" : "Ctrl";
const defaultHotkey = isMac ? "⌥ Space" : "Alt + Space";
function Key({ children }: { children: ReactNode }) { return <kbd>{children}</kbd>; }
function Badge() { return <span className="badge"><i /> tree · local</span>; }
function Brand() { return <div className="brand"><span className="mark">↗</span><div><strong>Offhand</strong><small>Your computer. A little more capable.</small></div></div>; }
function useAgentState() {
  const [event, setEvent] = useState<StateEvent>({ revision: 0, ui: { state: "idle" } });
  useEffect(() => {
    const unlisten = listen<StateEvent>("offhand://state", ({ payload }) => setEvent(old => payload.revision > old.revision ? payload : old));
    return () => { void unlisten.then(fn => fn()); };
  }, []);
  return event;
}
function Command({ ui }: { ui: UiState }) {
  const [command, setCommand] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => { if (ui.state === "listening") { setCommand(""); setError(""); setBusy(false); input.current?.focus(); } }, [ui.state]);
  useEffect(() => {
    const onError = (event: Event) => { setBusy(false); setError(String((event as CustomEvent).detail)); };
    if (window.__OFFHAND_ERROR__) setError(window.__OFFHAND_ERROR__);
    window.addEventListener("offhand-error", onError);
    return () => window.removeEventListener("offhand-error", onError);
  }, []);
  return <form className="cmdbar glass" onSubmit={e => {
    e.preventDefault();
    if (!command.trim() || busy) return;
    setBusy(true); setError("");
    void api.submit(command).catch(e => { setError(errorText(e)); setBusy(false); });
  }}>
    <div className="command-line"><span className="prompt-mark">↗</span><input ref={input} autoFocus aria-label="Command" placeholder="What should I do?" value={command} maxLength={8192} disabled={busy} onChange={e => setCommand(e.target.value)} onKeyDown={e => { if (e.key === "Escape") void api.stop().catch(e => setError(errorText(e))); }} /><Badge /></div>
    <div className="hint" role="status">{error ? <span className="error">{error}</span> : <><span><Key>↵</Key> run</span><span><Key>esc</Key> close</span><span className="command-scope">open · click · type · press</span></>}</div>
  </form>;
}
type Geometry = { x: number; y: number; scale: number; bottom: number };
declare global { interface Window { __OFFHAND_MONITOR__?: Geometry; __OFFHAND_ERROR__?: string } }
function Overlay({ event }: { event: StateEvent }) {
  const { ui } = event;
  const [geometry, setGeometry] = useState<Geometry>(window.__OFFHAND_MONITOR__ ?? { x: 0, y: 0, scale: 1, bottom: 20 });
  useEffect(() => {
    const update = (event: Event) => setGeometry((event as CustomEvent<Geometry>).detail);
    window.addEventListener("offhand-monitor", update);
    return () => window.removeEventListener("offhand-monitor", update);
  }, []);
  const target = "target" in ui ? ui.target : null;
  return <main className="overlay" aria-live="polite">
    {target && <div className={`ring ${ui.state === "await_confirm" ? "amber" : ui.state === "blocked" ? "red" : ""}`} style={{ left: (target.bounds.x - geometry.x) / geometry.scale - 6, top: (target.bounds.y - geometry.y) / geometry.scale - 6, width: target.bounds.w / geometry.scale + 12, height: target.bounds.h / geometry.scale + 12 }} />}
    {!['idle', 'listening', 'await_confirm', 'blocked'].includes(ui.state) && <div className={`pill glass ${ui.state}`} key={event.revision} style={{ bottom: geometry.bottom }}>
      {ui.state === "running" && <><span className="spin">◐</span><span>{ui.target?.label ?? "Working"} <span className="muted mono">{ui.step} of {ui.total}</span></span><Badge /><span className="divider" /><span className="muted"><Key>esc</Key> stop</span></>}
      {ui.state === "done" && <><span className="success">✓</span><span>{ui.summary}</span><div className="countdown" /></>}
      {ui.state === "stuck" && <><span className="muted">?</span><span className="state-message" title={ui.query}>{ui.query}</span><span className="muted"><Key>{primary} ↵</Key> retry · <Key>esc</Key> stop</span></>}
      {ui.state === "paused" && <><span className="muted">Ⅱ</span><span>Paused</span><span className="muted"><Key>{primary} ↵</Key> resume · <Key>esc</Key> stop</span></>}
    </div>}
  </main>;
}
function Confirmation({ ui }: { ui: UiState }) {
  const [error, setError] = useState("");
  useEffect(() => setError(""), [ui]);
  if (ui.state !== "await_confirm" && ui.state !== "blocked") return null;
  const blocked = ui.state === "blocked";
  const decide = (decision: "allow" | "skip" | "stop") => {
    void api.confirm(ui.state === "await_confirm" ? ui.confirmation_id : 0, decision).catch(e => setError(errorText(e)));
  };
  return <main className={`confirmation glass ${blocked ? "blocked" : ""}`} onKeyDown={e => { if (e.key === "Enter" && !e.ctrlKey && !e.metaKey) { e.preventDefault(); e.stopPropagation(); } }}>
    <div className="card-heading"><span className={`state-icon ${blocked ? "red" : "amber"}`}>{blocked ? "⊘" : "!"}</span><div><h2>{blocked ? "This action is blocked" : `Press “${ui.label}”?`}</h2><p>{ui.state === "await_confirm" ? ui.app : ui.label}</p></div></div>
    {ui.state === "await_confirm" && ui.detail && <p className="detail">{ui.detail}</p>}
    {blocked ? <><p className="detail">Permanent deletion is never automatic. Offhand has stopped here.</p><button className="button" onClick={() => decide("stop")}>OK <Key>esc</Key></button></> : <div className="actions"><button className="button allow" onClick={() => decide("allow")}>Allow <Key>{primary} ↵</Key></button><button className="button" onClick={() => decide("skip")}>Skip <Key>⇥</Key></button><button className="quiet" onClick={() => decide("stop")}>Stop <Key>esc</Key></button></div>}
    {error && <p className="error" role="alert">{error}</p>}
  </main>;
}
function PermissionRows({ permissions, request }: { permissions: Permissions | null; request: (p: "accessibility" | "screen_recording") => void }) {
  if (!isMac) return <p className="muted">{permissions ? "Nothing to grant. Windows uses its accessibility APIs directly." : "Checking permissions…"}</p>;
  return <div className="permission-list">{([['accessibility', 'Accessibility', 'Read controls and perform your commands.'], ['screen_recording', 'Screen Recording', 'Optional for future vision features. Not needed for tree commands.']] as const).map(([id, title, description]) => <div className="permission-row" key={id}><div><strong>{title}</strong><p>{description}</p></div>{permissions?.[id] === true ? <span className="success">✓ Granted</span> : <button className="button" onClick={() => request(id)}>{permissions?.[id] === null ? "Not required" : "Grant access"}</button>}</div>)}</div>;
}
function ModelCard() { return <><div className="model-line"><span className="model-icon">Q</span><div><strong>Qwen3.5-4B</strong><p className="mono">2.9 GB · local model</p></div><span className="badge">Unavailable</span></div><p className="muted">Model download unavailable in v0.1; tree commands work now.</p><div className="progress" role="progressbar" aria-valuenow={0} aria-valuemin={0} aria-valuemax={100}><span /></div><div className="model-footer"><span className="mono muted">0%</span><button className="button" disabled>Download model</button></div></>; }
function Preferences({ firstRun }: { firstRun: boolean }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [permissions, setPermissions] = useState<Permissions | null>(null);
  const [hotkey, setHotkey] = useState("Alt+Space");
  const hotkeyDirty = useRef(false);
  const [step, setStep] = useState(0);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const steps = isMac ? ["Hotkey", "Permissions", "Model", "Try it"] : ["Hotkey", "Model", "Try it"];
  const current = steps[step];
  useEffect(() => {
    const refresh = () => {
      if (document.visibilityState === "hidden") return;
      void api.settings().then(s => { setSettings(s); if (!hotkeyDirty.current) setHotkey(s.hotkey); }).catch(e => setError(errorText(e)));
    };
    refresh();
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => { window.removeEventListener("focus", refresh); document.removeEventListener("visibilitychange", refresh); };
  }, []);
  useEffect(() => {
    if (firstRun && current !== "Permissions") return;
    const poll = () => { if (document.visibilityState === "hidden") return; void api.permissions().then(setPermissions).catch(e => setError(errorText(e))); };
    poll(); const interval = setInterval(poll, 1000); return () => clearInterval(interval);
  }, [firstRun, current]);
  const request = (p: "accessibility" | "screen_recording") => { void api.requestPermission(p).then(setPermissions).catch(e => setError(errorText(e))); };
  const saveHotkey = async () => { setError(""); setSaving(true); try { const saved = await api.setHotkey(hotkey); setSettings(saved); setHotkey(saved.hotkey); hotkeyDirty.current = false; return true; } catch (e) { setError(errorText(e)); return false; } finally { setSaving(false); } };
  const hotkeyControl = <><p className="muted">One shortcut, from anywhere. {isMac ? "Option + Space by default." : "Alt + Space replaces the Windows system menu shortcut while Offhand runs."}</p><div className="hotkey-control"><input aria-label="Global shortcut" value={hotkey} onChange={e => { hotkeyDirty.current = true; setHotkey(e.target.value); }} spellCheck={false} /><button className="button" disabled={saving || !settings} onClick={() => { void saveHotkey(); }}>Save shortcut</button></div><small className="muted">Current: <span className="mono">{settings?.hotkey ?? "Checking…"}</span>. Use modifiers and a key, for example Control+Shift+Space. A conflicting shortcut keeps your current binding.</small></>;
  const advance = async () => {
    if (step === 0 && !(await saveHotkey())) return;
    if (step < steps.length - 1) setStep(step + 1);
    else { setSaving(true); void api.completeSetup().catch(e => { setError(errorText(e)); setSaving(false); }); }
  };
  return <main className={`preferences ${firstRun ? "onboarding" : "settings"}`}>
    <header><Brand />{!firstRun && <span className="version mono">v0.1</span>}</header>
    {firstRun ? <><nav className="steps" aria-label="Setup progress">{steps.map((s, i) => <span key={s} className={i === step ? "active" : i < step ? "complete" : ""}><b>{i < step ? "✓" : i + 1}</b>{s}</span>)}</nav><section className="setup-content">
      <div className="eyebrow">MAKE YOURSELF AT HOME</div><h1>{current === "Hotkey" ? "A hand, whenever you need it." : current === "Permissions" ? "A little permission to help." : current === "Model" ? "Private. Right on your computer." : "Give it something to do."}</h1>
      {current === "Hotkey" && hotkeyControl}
      {current === "Permissions" && <><p className="muted">Accessibility lets Offhand work with the controls you can see.</p><PermissionRows permissions={permissions} request={request} /></>}
      {current === "Model" && <ModelCard />}
      {current === "Try it" && <><p className="muted">Press <Key>{settings?.hotkey ?? defaultHotkey}</Key> from another app, then try:</p><div className="sample mono">{isMac ? "open Calculator" : "open notepad"}<Key>↵</Key></div><p className="muted">One command at a time: open an app, click a label, type text, or press a key. <Key>esc</Key> always stops a run.</p>{settings?.platform === "unsupported" && <p className="error">Desktop actions require macOS or Windows.</p>}</>}
    </section><footer><span className="muted">{step + 1} of {steps.length}</span><div className="actions">{step > 0 && <button className="quiet" onClick={() => setStep(step - 1)}>Back</button>}<button className="button primary" disabled={saving || !settings} onClick={() => { void advance(); }}>{step === steps.length - 1 ? "Start using Offhand" : "Continue"}<span>→</span></button></div></footer></> : <><div className="page-title"><h1>Settings</h1><p className="muted">Small controls. Yours to change.</p></div><div className="settings-grid"><section className="settings-card"><h2>Model</h2><ModelCard /></section><section className="settings-card"><h2>Hotkey</h2>{hotkeyControl}</section><section className="settings-card"><h2>Safety</h2><p>Risky actions ask first. Permanent deletion is blocked.</p><p className="muted">Allow with <Key>{primary} ↵</Key> or the Allow button. Plain Enter never approves. <Key>esc</Key> stops immediately. Pause and resume from the tray.</p><span className="badge">Always on</span></section><section className="settings-card"><h2>Cloud <span className="badge">Off</span></h2><p className="muted">Cloud inference is unavailable in v0.1. Commands stay on this computer.</p><label className="disabled-control"><input type="checkbox" disabled checked={false} readOnly /> Enable cloud fallback</label></section><section className="settings-card wide"><h2>Permissions</h2><PermissionRows permissions={permissions} request={request} /></section></div></>}
    {error && <p className="error preferences-error" role="alert">{error}</p>}
  </main>;
}
function AgentSurface() {
  const event = useAgentState();
  if (surface === "overlay") return <Overlay event={event} />;
  if (surface === "confirm") return <Confirmation ui={event.ui} />;
  return <Command ui={event.ui} />;
}
export default function App() {
  if (surface === "settings" || surface === "first-run") return <Preferences firstRun={surface === "first-run"} />;
  return <AgentSurface />;
}
