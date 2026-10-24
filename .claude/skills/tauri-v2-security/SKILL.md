---
name: tauri-v2-security
description: Use when you add a window, plugin, Tauri command, or capability file in app/, or change the CSP. Applies Tauri v2 least-privilege rules to Offhand.
---

# Tauri v2 security

The Offhand app has three windows: the command bar, the overlay, and settings. Each one gets only the permissions it needs. Reference: https://v2.tauri.app/security/capabilities/

## Capabilities

1. Put capability files in `app/src-tauri/capabilities/`.
2. Write one file per window label. Do not share one file across windows.
3. List the fewest permissions. Do not use wildcard permissions.
4. A new plugin grants nothing by default. Add each permission you need to the one window that needs it.

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "overlay",
  "windows": ["overlay"],
  "permissions": [
    "core:window:allow-set-ignore-cursor-events",
    "core:window:allow-set-content-protected"
  ]
}
```

## Overlay window

The overlay must never appear in the agent's own captures, and must let clicks through. Set both at creation.

```rust
window.set_ignore_cursor_events(true)?;
window.set_content_protected(true)?;
```

Test both on macOS and on Windows. Content protection behaves differently per OS, so record gaps in `docs/platform-matrix.md`.

## Remote content and CSP

- Load no remote URLs in any window. Bundle all assets.
- Set a strict `app.security.csp` in `tauri.conf.json`: `default-src 'self'`, no `unsafe-eval`, no inline scripts.
- Do not set `dangerousRemoteDomainIpcAccess`.
- Only the updater and `xtask fetch-model` may use the network. Everything else goes through `offhand-cloud`, if enabled.

## Commands

Treat every argument from the front end as untrusted.

```rust
#[tauri::command]
fn set_model(id: String) -> Result<(), AppError> {
    let id = ModelId::parse(&id)?; // reject unknown ids
    // ...
    Ok(())
}
```

- Validate and parse input into types.
- Return `Result` with a typed error. Do not panic.
- Do not expose a command that runs an action. Send intent to the core. The core goes through `offhand-policy`.
- Store API keys in the OS keychain. Never in local storage or logs.

## Checklist

- [ ] One capability file per window label
- [ ] No wildcard or unused permissions
- [ ] No remote URLs, strict CSP
- [ ] Overlay sets both cursor-ignore and content-protected
- [ ] Commands validate input and return typed errors
- [ ] No key in the front end
- [ ] `safety-reviewer` reviewed capability or CSP changes
