use std::path::Path;

#[tauri::command]
fn launch_agent(agent: String, cwd: String) -> Result<String, String> {
    let path = Path::new(&cwd);
    if !path.exists() {
        return Err(format!("project path does not exist: {}", cwd));
    }

    if !path.is_dir() {
        return Err(format!("project path is not a directory: {}", cwd));
    }

    println!("[launch_agent] agent={}, cwd={}", agent, cwd);
    Ok(format!("launch request accepted for {}", agent))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![launch_agent])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
