// GlassDock: uma dock de vidro com widgets no lugar da barra de tarefas do
// Windows. App nativo (Win32 + Direct2D), feito para gastar o mínimo de
// recursos: parado, não redesenha nada e só acorda com eventos do sistema.
#![windows_subsystem = "windows"]

mod app;
mod config;
mod dock;
mod dragdrop;
mod gfx;
mod glass;
mod icons;
mod media;
mod net;
mod popup;
mod settings;
mod status;
mod sysmon;
mod taskbar;
mod tray;
mod ui;
mod util;
mod widgets;
mod winlist;

use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::System::Ole::OleInitialize;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--watchdog" {
        if let Ok(pid) = args[2].parse::<u32>() {
            taskbar::watchdog_main(pid);
        }
        return;
    }
    if args.len() >= 2 && args[1] == "--claude-statusline" {
        widgets::ai::statusline_main();
        return;
    }
    if args.len() >= 3 && args[1] == "--render-test" {
        app::DUMP.with(|d| *d.borrow_mut() = Some(args[2].clone()));
    }
    if args.len() >= 2 && args[1] == "--restore-taskbar" {
        taskbar::recover_on_start();
        return;
    }
    unsafe {
        let _m = CreateMutexW(None, true, w!("GlassDock.SingleInstance.v1"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            // já está rodando: abre os Ajustes da instância existente
            if let Ok(h) = FindWindowW(w!("GlassDockWnd"), w!("GlassDockMain")) {
                let _ = PostMessageW(Some(h), app::WM_APP_SHOW_SETTINGS, WPARAM(0), LPARAM(0));
            }
            return;
        }
        // erros inesperados vão para um log em vez de passar em silêncio
        std::panic::set_hook(Box::new(|info| {
            let msg = format!("[{}] {}\n", util::unix_now(), info);
            let p = util::data_dir().join("erros.log");
            let old = std::fs::read_to_string(&p).unwrap_or_default();
            let keep: String = old.chars().rev().take(20000).collect::<Vec<_>>().into_iter().rev().collect();
            let _ = std::fs::write(&p, keep + &msg);
        }));
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = OleInitialize(None);
        app::run();
        std::mem::forget(_m);
    }
}
