// Release builds on Windows get no extra console window. Do not remove.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    telex_lib::run();
}
