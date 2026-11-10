// Janelas abertas: quais aparecem na dock, a que app e a que ambiente
// pertencem, e as ações (ativar, minimizar, fechar, trocar de ambiente).
use std::collections::HashMap;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::PropertiesSystem::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::PinnedApp;
use crate::util::*;

#[derive(Clone, Debug)]
pub struct Win {
    pub hwnd: HWND,
    pub pid: u32,
    pub exe: String,
    pub aumid: String,
    pub key: String,
    pub ws: u32,
    pub min_by_us: bool,
    pub last_active: u64,
    pub monitor: isize,
    pub flashing: bool,
    /// quando o app viu esta janela pela primeira vez (ms)
    pub seen: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct SavedMap {
    wins: Vec<(isize, u32, u32, bool)>, // hwnd, pid, ws, minimizada por nós
}

pub struct WinList {
    pub wins: Vec<Win>,
    pub fg: HWND,
    saved: HashMap<isize, (u32, u32, bool)>,
    vdm: Option<IVirtualDesktopManager>,
    pub own_pid: u32,
    pub dirty_save: bool,
    pub switching_until: u64,
}

pub fn group_key(exe: &str, aumid: &str) -> String {
    if !aumid.is_empty() {
        format!("id:{}", aumid.to_lowercase())
    } else {
        format!("exe:{}", exe.to_lowercase())
    }
}

fn file_name_lower(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn parent_lower(p: &str) -> String {
    std::path::Path::new(p).parent().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Só letras e números, minúsculo: "Google Chrome" == "googlechrome".
pub fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

fn is_updater(exe: &str) -> bool {
    file_name_lower(exe) == "update.exe"
}

/// Apps que abrem por um atualizador (Figma, Discord, Slack…): o atalho aponta
/// para `Update.exe --processStart App.exe`, mas a janela é de `App.exe`.
fn process_start_name(app: &PinnedApp) -> Option<String> {
    let a = app.lnk_args.to_lowercase();
    let i = a.find("--processstart")?;
    let rest = a[i..].split_whitespace().nth(1)?;
    Some(rest.trim_matches('"').to_string())
}

fn is_frame_host(w: &Win) -> bool {
    w.exe.to_lowercase().ends_with("applicationframehost.exe")
}

/// A janela é deste app fixado (critérios seguros).
pub fn app_matches(app: &PinnedApp, w: &Win) -> bool {
    if !app.aumid.is_empty() && !w.aumid.is_empty() && app.aumid.eq_ignore_ascii_case(&w.aumid) {
        return true;
    }
    // Apps hospedados no ApplicationFrameHost não casam pelo exe.
    if is_frame_host(w) || w.exe.is_empty() {
        return false;
    }
    let we = w.exe.to_lowercase();
    if !app.exe.is_empty() && app.exe.eq_ignore_ascii_case(&w.exe) {
        return true;
    }
    // aprendido ao abrir: o caminho exato, ou o mesmo nome de arquivo (o app
    // pode ter se atualizado para uma pasta nova, como app-1.2.3 → app-1.2.4)
    if app.alt.iter().any(|a| *a == we) {
        return true;
    }
    let wn = file_name_lower(&w.exe);
    if app.alt.iter().any(|a| file_name_lower(a) == wn) {
        return true;
    }
    if !app.exe.is_empty() && is_updater(&app.exe) {
        if let Some(name) = process_start_name(app) {
            if file_name_lower(&w.exe) == name {
                return true;
            }
        }
        // a janela vive dentro da pasta do app (…\Figma\app-1.2.3\Figma.exe)
        let dir = parent_lower(&app.exe);
        if dir.len() > 8 && we.starts_with(&format!("{}\\", dir)) {
            return true;
        }
    }
    false
}

/// Critérios mais soltos, usados só para fixados que ficaram sem janela:
/// mesmo nome de arquivo em outra pasta, ou o mesmo nome do app.
pub fn app_matches_loose(app: &PinnedApp, w: &Win) -> bool {
    if is_frame_host(w) || w.exe.is_empty() {
        return false;
    }
    if !app.exe.is_empty() && !is_updater(&app.exe) && file_name_lower(&app.exe) == file_name_lower(&w.exe) {
        return true;
    }
    let n = norm(&app.name);
    if n.len() < 3 {
        return false;
    }
    if n == norm(&file_stem(&w.exe)) {
        return true;
    }
    let disp = crate::icons::app_display_name(&w.exe, &w.aumid);
    n == norm(&disp)
}

fn saved_path() -> std::path::PathBuf {
    data_dir().join("windows.json")
}

impl WinList {
    pub fn new() -> WinList {
        let saved: SavedMap = read_json(&saved_path()).unwrap_or_default();
        let vdm = unsafe { CoCreateInstance(&VirtualDesktopManager, None, CLSCTX_ALL).ok() };
        WinList {
            wins: vec![],
            fg: HWND::default(),
            saved: saved.wins.into_iter().map(|(h, p, w, m)| (h, (p, w, m))).collect(),
            vdm,
            own_pid: unsafe { GetCurrentProcessId() },
            dirty_save: false,
            switching_until: 0,
        }
    }

    pub fn save(&mut self) {
        let m = SavedMap { wins: self.wins.iter().map(|w| (w.hwnd.0 as isize, w.pid, w.ws, w.min_by_us)).collect() };
        write_json_atomic(&saved_path(), &m);
        self.dirty_save = false;
    }

    pub fn is_taskbar_window(&self, h: HWND) -> bool {
        let r = self.is_taskbar_window_inner(h);
        if std::env::var("GLASSDOCK_DEBUG").is_ok() && unsafe { IsWindowVisible(h).as_bool() } {
            let line = format!("{:?} '{}' -> {}\n", h.0, window_title(h), r);
            let p = data_dir().join("debug.log");
            let mut s = std::fs::read_to_string(&p).unwrap_or_default();
            s.push_str(&line);
            let _ = std::fs::write(&p, s);
        }
        r
    }

    fn is_taskbar_window_inner(&self, h: HWND) -> bool {
        unsafe {
            if !IsWindowVisible(h).as_bool() {
                return false;
            }
            let ex = WINDOW_EX_STYLE(GetWindowLongW(h, GWL_EXSTYLE) as u32);
            // janelas "visíveis" que na prática não aparecem (apps de fundo):
            // tamanho zero, fora de todas as telas ou totalmente transparentes
            if !IsIconic(h).as_bool() {
                let mut r = RECT::default();
                let _ = GetWindowRect(h, &mut r);
                if r.right - r.left <= 1 || r.bottom - r.top <= 1 {
                    return false;
                }
                let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
                let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
                let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
                let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
                if r.right <= vx || r.bottom <= vy || r.left >= vx + vw || r.top >= vy + vh {
                    return false;
                }
            }
            if ex.contains(WS_EX_LAYERED) {
                let mut alpha = 255u8;
                let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
                if GetLayeredWindowAttributes(h, None, Some(&mut alpha), Some(&mut flags)).is_ok() && flags.contains(LWA_ALPHA) && alpha == 0 {
                    return false;
                }
            }
            let owner = GetWindow(h, GW_OWNER).unwrap_or_default();
            let appwin = ex.contains(WS_EX_APPWINDOW);
            if ex.contains(WS_EX_TOOLWINDOW) && !appwin {
                return false;
            }
            if !owner.0.is_null() && !appwin {
                return false;
            }
            if ex.contains(WS_EX_NOACTIVATE) && !appwin {
                return false;
            }
            let mut cloaked: u32 = 0;
            let _ = DwmGetWindowAttribute(h, DWMWA_CLOAKED, &mut cloaked as *mut _ as _, 4);
            if cloaked != 0 {
                // Ocultada pelo Shell: outra área de trabalho virtual ou app suspenso
                return false;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid == self.own_pid {
                return false;
            }
            let mut cls = [0u16; 64];
            let n = GetClassNameW(h, &mut cls);
            let cls = String::from_utf16_lossy(&cls[..n as usize]);
            if matches!(cls.as_str(), "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Progman" | "WorkerW") {
                return false;
            }
            if GetWindowTextLengthW(h) == 0 && cls != "ApplicationFrameWindow" {
                return false;
            }
            if let Some(vdm) = &self.vdm {
                if let Ok(on) = vdm.IsWindowOnCurrentVirtualDesktop(h) {
                    if !on.as_bool() {
                        if std::env::var("GLASSDOCK_DEBUG").is_ok() {
                            let _ = std::fs::write(data_dir().join("debug_vdm.log"), "vdm false");
                        }
                        return false;
                    }
                }
            }
            true
        }
    }

    /// Reenumera as janelas. `assign` dá o ambiente de uma janela nova a partir do monitor.
    pub fn refresh(&mut self, assign: &dyn Fn(isize) -> u32) -> bool {
        unsafe extern "system" fn cb(h: HWND, l: LPARAM) -> BOOL {
            let v = &mut *(l.0 as *mut Vec<HWND>);
            v.push(h);
            TRUE
        }
        let mut all: Vec<HWND> = vec![];
        unsafe {
            let _ = EnumWindows(Some(cb), LPARAM(&mut all as *mut _ as isize));
        }
        let mut changed = false;
        let mut next: Vec<Win> = Vec::with_capacity(all.len().min(64));
        let mut order = 0u64;
        for h in all {
            if !self.is_taskbar_window(h) {
                continue;
            }
            order += 1;
            let mon = unsafe { MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST).0 as isize };
            if let Some(i) = self.wins.iter().position(|w| w.hwnd == h) {
                let mut w = self.wins[i].clone();
                if w.monitor != mon {
                    w.monitor = mon;
                    changed = true;
                }
                if w.aumid.is_empty() && w.exe.to_lowercase().ends_with("applicationframehost.exe") {
                    let a = window_aumid(h);
                    if !a.is_empty() {
                        w.aumid = a;
                        w.key = group_key(&w.exe, &w.aumid);
                        changed = true;
                    }
                }
                next.push(w);
                continue;
            }
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
            let exe = process_image_path(pid).unwrap_or_default();
            let aumid = window_aumid(h);
            let key = group_key(&exe, &aumid);
            let (ws, min_by_us) = match self.saved.get(&(h.0 as isize)) {
                Some((p, ws, m)) if *p == pid => (*ws, *m),
                _ => (assign(mon), false),
            };
            next.push(Win {
                hwnd: h,
                pid,
                exe,
                aumid,
                key,
                ws,
                min_by_us,
                last_active: tick_ms().saturating_sub(order),
                monitor: mon,
                flashing: false,
                seen: tick_ms(),
            });
            changed = true;
        }
        if next.len() != self.wins.len() {
            changed = true;
        }
        // as que sumiram já foram descartadas; o mapa salvo só vale na partida
        self.saved.clear();
        // mantém a ordem de chegada (estável) para a dock não embaralhar
        let mut ordered: Vec<Win> = vec![];
        for w in &self.wins {
            if let Some(n) = next.iter().position(|x| x.hwnd == w.hwnd) {
                ordered.push(next.remove(n));
            }
        }
        ordered.extend(next);
        self.wins = ordered;
        if changed {
            self.dirty_save = true;
        }
        changed
    }

    pub fn get(&self, h: HWND) -> Option<&Win> {
        self.wins.iter().find(|w| w.hwnd == h)
    }

    pub fn get_mut(&mut self, h: HWND) -> Option<&mut Win> {
        self.wins.iter_mut().find(|w| w.hwnd == h)
    }

    pub fn note_foreground(&mut self, h: HWND) {
        self.fg = h;
        if let Some(w) = self.get_mut(h) {
            w.last_active = tick_ms();
            w.flashing = false;
            if w.min_by_us {
                w.min_by_us = false;
            }
        }
    }
}

pub fn window_aumid(h: HWND) -> String {
    unsafe {
        let Ok(store) = SHGetPropertyStoreForWindow::<IPropertyStore>(h) else { return String::new() };
        let Ok(pv) = store.GetValue(&PKEY_AppUserModel_ID) else { return String::new() };
        match PropVariantToStringAlloc(&pv) {
            Ok(p) => {
                let s = from_pwstr(p);
                CoTaskMemFree(Some(p.0 as _));
                s
            }
            Err(_) => String::new(),
        }
    }
}

pub fn window_title(h: HWND) -> String {
    unsafe {
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(h, &mut buf);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

pub fn is_minimized(h: HWND) -> bool {
    unsafe { IsIconic(h).as_bool() }
}

/// Traz a janela para a frente de forma confiável.
pub fn activate(h: HWND) {
    unsafe {
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        if SetForegroundWindow(h).as_bool() {
            return;
        }
        // Truque do Alt: libera a troca de foco para este processo.
        let inputs = [key_input(VK_MENU, false), key_input(VK_MENU, true)];
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        let _ = SetForegroundWindow(h);
        let fg = GetForegroundWindow();
        if fg != h {
            let cur = GetCurrentThreadId();
            let other = GetWindowThreadProcessId(fg, None);
            if other != 0 && other != cur {
                let _ = AttachThreadInput(cur, other, true);
                let _ = BringWindowToTop(h);
                let _ = SetForegroundWindow(h);
                let _ = AttachThreadInput(cur, other, false);
            }
        }
    }
}

pub fn key_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

pub fn send_keys(down: &[VIRTUAL_KEY]) {
    let mut v: Vec<INPUT> = down.iter().map(|k| key_input(*k, false)).collect();
    v.extend(down.iter().rev().map(|k| key_input(*k, true)));
    unsafe {
        SendInput(&v, std::mem::size_of::<INPUT>() as i32);
    }
}

pub fn minimize(h: HWND) {
    unsafe {
        let _ = ShowWindow(h, SW_MINIMIZE);
    }
}

/// Minimiza sem ativar; síncrono quando o app responde, assíncrono se travado.
pub fn minimize_quiet(h: HWND) {
    unsafe {
        if IsHungAppWindow(h).as_bool() {
            let _ = ShowWindowAsync(h, SW_SHOWMINNOACTIVE);
        } else {
            let _ = ShowWindow(h, SW_SHOWMINNOACTIVE);
        }
    }
}

pub fn restore_quiet(h: HWND) {
    unsafe {
        if IsHungAppWindow(h).as_bool() {
            let _ = ShowWindowAsync(h, SW_SHOWNOACTIVATE);
        } else {
            let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
        }
    }
}

pub fn close(h: HWND) {
    unsafe {
        let _ = PostMessageW(Some(h), WM_SYSCOMMAND, WPARAM(SC_CLOSE as usize), LPARAM(0));
    }
}

pub fn kill_process(pid: u32) {
    unsafe {
        if let Ok(hp) = OpenProcess(PROCESS_TERMINATE, false, pid) {
            let _ = TerminateProcess(hp, 1);
            let _ = CloseHandle(hp);
        }
    }
}

pub fn launch(app: &PinnedApp, new_window_hint: bool) {
    let target = if !app.launch.is_empty() { app.launch.clone() } else { app.exe.clone() };
    let mut args = app.args.clone();
    if new_window_hint && args.is_empty() {
        let e = app.exe.to_lowercase();
        if e.ends_with("chrome.exe") || e.ends_with("msedge.exe") || e.ends_with("brave.exe") || e.ends_with("firefox.exe") {
            args = "--new-window".into();
        }
    }
    // Abre numa thread à parte: o Windows pode levar um tempo para resolver
    // o atalho, e a dock continua respondendo (a animação começa na hora).
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }
    let name = app.name.clone();
    std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let ok = if args.is_empty() { shell_exec_blocking(&target, None, None) } else { shell_exec_blocking(&target, Some(&args), None) };
        CoUninitialize();
        if !ok {
            if let Ok(mut q) = LAUNCH_FAILS.lock() {
                q.push(name);
            }
            crate::app::post(crate::app::WM_APP_LAUNCHFAIL, 0, 0);
        }
    });
}

/// Apps que o Windows não conseguiu abrir (a thread principal mostra o aviso).
pub static LAUNCH_FAILS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
