fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "offhand_submit",
                "offhand_stop",
                "offhand_confirm",
                "offhand_settings",
                "offhand_set_hotkey",
                "offhand_permissions",
                "offhand_request_permission",
                "offhand_complete_setup",
            ]),
        ),
    )
    .expect("failed to build the Offhand application manifest");
}
