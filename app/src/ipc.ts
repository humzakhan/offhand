import { invoke } from "@tauri-apps/api/core";

export type Bounds = { x: number; y: number; w: number; h: number };
export type Target = { label: string; bounds: Bounds };
export type UiState =
  | { state: "idle" | "listening" | "paused" }
  | { state: "running"; step: number; total: number; target: Target | null }
  | { state: "await_confirm"; confirmation_id: number; label: string; app: string; detail: string | null; target: Target | null }
  | { state: "blocked"; label: string; target: Target | null }
  | { state: "stuck"; query: string; cloud_available: boolean }
  | { state: "done"; summary: string; undoable: boolean };
export type StateEvent = { revision: number; ui: UiState; armed: boolean };
export type Permissions = { accessibility: boolean | null; screen_recording: boolean | null };
export type Settings = {
  platform: "macos" | "windows" | "unsupported";
  hotkey: string;
  first_run_complete: boolean;
  model: { id: "qwen3.5-4b"; status: "unavailable"; progress: 0 };
  cloud_enabled: false;
};
export type AppError = { code: string; message: string };
export const api = {
  submit: (command: string) => invoke<null>("offhand_submit", { command }),
  stop: () => invoke<null>("offhand_stop"),
  confirm: (confirmation_id: number, decision: "allow" | "skip" | "stop") => invoke<null>("offhand_confirm", { confirmation_id, decision }),
  settings: () => invoke<Settings>("offhand_settings"),
  setHotkey: (hotkey: string) => invoke<Settings>("offhand_set_hotkey", { hotkey }),
  permissions: () => invoke<Permissions>("offhand_permissions"),
  requestPermission: (permission: "accessibility" | "screen_recording") => invoke<Permissions>("offhand_request_permission", { permission }),
  completeSetup: () => invoke<null>("offhand_complete_setup"),
};
export function errorText(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) return String(error.message);
  return String(error);
}
