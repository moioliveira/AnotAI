// Sem janela de console no build de release do Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    anotai_desktop_lib::run();
}
