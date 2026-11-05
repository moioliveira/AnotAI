// Declara os comandos do app: assim cada um vira uma permissão e só as
// janelas que a recebem em `capabilities/` podem chamá-lo (o overlay não
// recebe nenhuma — Tech Spec 6.3).
const COMMANDS: &[&str] = &[
    "app_info",
    "get_settings",
    "save_settings",
    "set_mode",
    "api_key_status",
    "set_api_key",
    "history_list",
    "history_delete",
    "delete_all_data",
    "export_data",
    "dictionary_list",
    "dictionary_add",
    "dictionary_remove",
    "stats",
    "models_status",
    "model_download",
    "model_delete",
    "mic_test",
    "set_cloud_consent",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .unwrap_or_else(|e| panic!("falha no tauri-build: {e}"));
}
