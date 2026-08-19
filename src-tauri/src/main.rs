// エントリポイントのみ。実体はlib.rsに委譲する（architecture.md 1章）。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    ledge_lib::run();
}
