// Barra de tarefas do Windows: esconder enquanto o app roda e devolver
// exatamente como estava ao sair, inclusive se o app fechar de repente
// (um processo vigia espera o principal terminar, sem gastar CPU).
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::TaskbarMode;
use crate::util::*;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct State {
    /// estado original do auto-ocultar da barra
    orig_autohide: bool,
    /// estamos mexendo na barra agora
    active: bool,
    /// janelas que minimizamos ao trocar de ambiente (para devolver)
    minimized: Vec<isize>,
}

fn state_path() -> std::path::PathBuf {
    data_dir().join("taskbar.json")
}

pub fn tray_windows() -> Vec<HWND> {
    let mut v = vec![];
    unsafe {
        let own = GetCurrentProcessId();
        for cls in [w!("Shell_TrayWnd"), w!("Shell_SecondaryTrayWnd")] {
            let mut after: Option<HWND> = None;
            loop {
                let h = FindWindowExW(None, after, cls, None).unwrap_or_default();
                if h.0.is_null() {
                    break;
                }
                let mut pid = 0;
                GetWindowThreadProcessId(h, Some(&mut pid));
                if pid != own {
                    v.push(h);
                }
                after = Some(h);
            }
        }
    }
    v
}

pub fn explorer_tray() -> HWND {
    tray_windows().into_iter().next().unwrap_or_default()
}

fn appbar_state() -> u32 {
    unsafe {
        let mut abd = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, ..Default::default() };
        SHAppBarMessage(ABM_GETSTATE, &mut abd) as u32
    }
}

fn set_appbar_autohide(on: bool) {
    unsafe {
        let mut abd = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, ..Default::default() };
        abd.hWnd = explorer_tray();
        abd.lParam = LPARAM(if on { ABS_AUTOHIDE as isize } else { ABS_ALWAYSONTOP as isize });
        SHAppBarMessage(ABM_SETSTATE, &mut abd);
    }
}

pub struct Taskbar {
    pub mode: TaskbarMode,
    st: State,
}

impl Taskbar {
    pub fn new() -> Taskbar {
        let st: State = read_json(&state_path()).unwrap_or_default();
        Taskbar { mode: TaskbarMode::Keep, st }
    }

    fn save(&self) {
        write_json_atomic(&state_path(), &self.st);
    }

    pub fn apply(&mut self, mode: TaskbarMode) {
        if !self.st.active && mode != TaskbarMode::Keep {
            self.st.orig_autohide = appbar_state() & ABS_AUTOHIDE != 0;
            self.st.active = true;
            self.save();
        }
        self.mode = mode;
        match mode {
            TaskbarMode::Hide => {
                set_appbar_autohide(true);
                self.hide_now();
            }
            TaskbarMode::AutoHide => {
                set_appbar_autohide(true);
                for h in tray_windows() {
                    unsafe {
                        let _ = ShowWindow(h, SW_SHOWNA);
                    }
                }
            }
            TaskbarMode::Keep => self.restore(),
        }
    }

    pub fn hide_now(&self) {
        if self.mode != TaskbarMode::Hide {
            return;
        }
        for h in tray_windows() {
            unsafe {
                if IsWindowVisible(h).as_bool() {
                    let _ = ShowWindow(h, SW_HIDE);
                }
            }
        }
    }

    pub fn note_minimized(&mut self, list: Vec<isize>) {
        if self.st.minimized != list {
            self.st.minimized = list;
            self.save();
        }
    }

    pub fn restore(&mut self) {
        restore_from(&self.st);
        self.st.active = false;
        self.st.minimized.clear();
        self.save();
    }
}

fn restore_from(st: &State) {
    if st.active {
        set_appbar_autohide(st.orig_autohide);
        for h in tray_windows() {
            unsafe {
                let _ = ShowWindow(h, SW_SHOWNA);
            }
        }
    }
    for h in &st.minimized {
        unsafe {
            let hw = HWND(*h as _);
            if IsWindow(Some(hw)).as_bool() && IsIconic(hw).as_bool() {
                let _ = ShowWindowAsync(hw, SW_SHOWNOACTIVATE);
            }
        }
    }
}

fn spawn_self(args: &str, flags: PROCESS_CREATION_FLAGS) -> bool {
    unsafe {
        let exe = exe_path().to_string_lossy().to_string();
        let mut cmd = wide(&format!("\"{}\" {}", exe, args));
        let si = STARTUPINFOW { cb: std::mem::size_of::<STARTUPINFOW>() as u32, ..Default::default() };
        let mut pi = PROCESS_INFORMATION::default();
        let ok = CreateProcessW(None, Some(PWSTR(cmd.as_mut_ptr())), None, None, false, flags, None, None, &si, &mut pi).is_ok();
        if ok {
            let _ = CloseHandle(pi.hProcess);
            let _ = CloseHandle(pi.hThread);
        }
        ok
    }
}

/// Inicia o vigia. Ele nasce por um processo intermediário que fecha na hora,
/// assim não fica "pendurado" no GlassDock: o Gerenciador de Tarefas não o
/// encerra junto com a dock e a barra do Windows volta mesmo nesse caso.
pub fn spawn_watchdog() {
    let pid = unsafe { GetCurrentProcessId() };
    let base = CREATE_NO_WINDOW | DETACHED_PROCESS;
    let arg = format!("--watchdog-launch {}", pid);
    if !spawn_self(&arg, base | CREATE_BREAKAWAY_FROM_JOB) {
        spawn_self(&arg, base);
    }
}

/// O processo intermediário: lança o vigia de verdade e sai.
pub fn watchdog_launch(pid: u32) {
    let base = CREATE_NO_WINDOW | DETACHED_PROCESS;
    let arg = format!("--watchdog {}", pid);
    if !spawn_self(&arg, base | CREATE_BREAKAWAY_FROM_JOB) {
        spawn_self(&arg, base);
    }
}

/// Corpo do vigia: espera o processo principal terminar e, se ele não
/// devolveu a barra, devolve aqui. Se a dock travar (não responde), mostra a
/// barra do Windows enquanto isso, para nunca ficar sem barra nenhuma.
pub fn watchdog_main(pid: u32) {
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            let mut hung = 0;
            loop {
                if WaitForSingleObject(h, 4000) != WAIT_TIMEOUT {
                    break;
                }
                let main = FindWindowW(w!("GlassDockWnd"), w!("GlassDockMain")).unwrap_or_default();
                if !main.0.is_null() && IsHungAppWindow(main).as_bool() {
                    hung += 1;
                    if hung == 3 {
                        for t in tray_windows() {
                            let _ = ShowWindow(t, SW_SHOWNA);
                        }
                    }
                } else {
                    hung = 0;
                }
            }
            let _ = CloseHandle(h);
        }
        let st: State = read_json(&state_path()).unwrap_or_default();
        if st.active || !st.minimized.is_empty() {
            restore_from(&st);
            let clean = State { orig_autohide: st.orig_autohide, active: false, minimized: vec![] };
            write_json_atomic(&state_path(), &clean);
        }
    }
}

/// Recupera a barra se o app anterior caiu e o vigia não rodou.
pub fn recover_on_start() {
    let st: State = read_json(&state_path()).unwrap_or_default();
    if st.active {
        restore_from(&st);
        let clean = State { orig_autohide: st.orig_autohide, active: false, minimized: vec![] };
        write_json_atomic(&state_path(), &clean);
    }
}

pub fn open_start_menu() {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_LWIN;
    crate::winlist::send_keys(&[VK_LWIN]);
}

pub fn open_quick_settings() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_A, VK_LWIN};
    crate::winlist::send_keys(&[VK_LWIN, VK_A]);
}

pub fn open_notification_center() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LWIN, VK_N};
    crate::winlist::send_keys(&[VK_LWIN, VK_N]);
}

/// Liga/desliga "abrir com o Windows".
pub fn set_autostart(on: bool) {
    use windows::Win32::System::Registry::*;
    unsafe {
        let key = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let mut hk = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, pcw(&key), None, KEY_SET_VALUE, &mut hk).is_err() {
            return;
        }
        let name = wide("GlassDock");
        if on {
            let v = wide(&format!("\"{}\"", exe_path().to_string_lossy()));
            let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2);
            let _ = RegSetValueExW(hk, pcw(&name), None, REG_SZ, Some(bytes));
        } else {
            let _ = RegDeleteValueW(hk, pcw(&name));
        }
        let _ = RegCloseKey(hk);
    }
}
