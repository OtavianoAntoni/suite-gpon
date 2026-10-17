// Windows: sem janela de console em release (o log vai para arquivo via tauri-plugin-log).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    suite_gpon_lib::run();
}
