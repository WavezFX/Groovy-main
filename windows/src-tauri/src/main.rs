// Groovy runs without a console window: Groovy is the whole UI.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    groovy_lib::run()
}
