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
| Hotkey | planned (⌥Space) | planned (Alt+Space) | Alt is the Windows equivalent of ⌥. It overrides the window system menu, so first run lets the user change it |
| Permissions step in first run | planned (Accessibility, Screen Recording) | none | Windows has no grants to request, so first run has three steps instead of four |
| Tray position | planned (menu bar, top right) | planned (notification area, bottom right) | Set by the OS. The status pill sits 20 px above the screen edge on macOS and above the taskbar on Windows |
| Window material | planned (vibrancy) | planned (Mica or Acrylic) | Each OS has its own translucent material. Both need an opaque fallback |
