---
name: macos-permissions
description: Use when you work on offhand-macos or the app's onboarding and need Accessibility or Screen Recording access, code signing, or TCC debugging on macOS.
---

# macOS permissions

Offhand needs two TCC grants on macOS: Accessibility (read the tree, send input) and Screen Recording (capture windows). Without them, calls fail or return empty data.

## Stable signing identity

TCC ties a grant to the app's code signature. An ad hoc signature (`codesign -s -`) changes with each build, so macOS drops the grant after every rebuild.

1. Create a self-signed code-signing certificate in Keychain Access, or use a Developer ID.
2. Sign every dev build with the same identity.
3. Keep the bundle identifier fixed.

```sh
codesign --force --deep --sign "Offhand Dev" --identifier dev.offhand.app target/debug/bundle/Offhand.app
codesign -dv --verbose=2 Offhand.app   # check Identifier and Authority
```

## Reset grants in dev

Reset when a grant is stuck or after you change the identity.

```sh
tccutil reset Accessibility dev.offhand.app
tccutil reset ScreenCapture dev.offhand.app
```

Then launch the app and grant again in System Settings, Privacy and Security.

## Detect missing grants

Check before the first act and at start-up. Do not wait for a failure.

```rust
// SAFETY: AXIsProcessTrusted takes no arguments and has no preconditions.
let ax_ok = unsafe { AXIsProcessTrusted() };
// SAFETY: CGPreflightScreenCaptureAccess takes no arguments and does not prompt.
let sr_ok = unsafe { CGPreflightScreenCaptureAccess() };
```

Return a typed `PlatformError::PermissionMissing(Permission::Accessibility)`. Never fail silently. An unprivileged `CGEvent` post drops events with no error.

## Guide the user

1. Say which grant is missing and why Offhand needs it.
2. Open the right pane: `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility` or `?Privacy_ScreenCapture`.
3. Poll for the grant. Tell the user when it works.
4. Note that Screen Recording may need an app restart after the grant.

## Notarization basics

A release build needs a Developer ID certificate, the hardened runtime, and notarization.

```sh
codesign --force --options runtime --sign "Developer ID Application: NAME (TEAMID)" Offhand.app
xcrun notarytool submit Offhand.zip --keychain-profile offhand --wait
xcrun stapler staple Offhand.app
```

Never store the certificate or the notary password in the repository.

## Checklist

- [ ] Dev builds use one stable signing identity
- [ ] Both grants are checked at start-up
- [ ] A missing grant returns a typed error
- [ ] The onboarding opens the correct pane
- [ ] Every `unsafe` block has a `// SAFETY:` comment
- [ ] Release builds are signed, hardened, notarized, and stapled
