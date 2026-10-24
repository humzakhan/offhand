# Platform matrix

This table records each capability on macOS and Windows. A row that is not
equal on both platforms needs a reason in the Notes column. TODO: `cargo xtask
parity-check` is planned. Once it exists, it will fail if a row has a value in
one column only.

| Capability | macOS | Windows | Notes |
|---|---|---|---|
| Read UI tree | planned (AXUIElement) | planned (UI Automation) | Chromium on Windows needs accessibility on |
| Capture window | planned (ScreenCaptureKit) | planned (Windows.Graphics.Capture) | |
| Press element | planned (AXPress) | planned (Invoke pattern) | |
| Click at point | planned (CGEvent) | planned (SendInput) | SendInput fails into higher-integrity processes |
| Type text | planned (CGEvent) | planned (SendInput) | |
| Window-change events | planned (AXObserver) | planned (UIA events) | |
| Overlay hidden from capture | planned (Tauri) | planned (Tauri) | Week-1 spike |
| GPU inference | planned (Metal) | planned (CUDA, Vulkan) | |
| NPU inference | not planned | stretch | Snapdragon only, if tested |
| Permissions prompt | Accessibility, Screen Recording | none for UIA | macOS grants need a stable signature |
