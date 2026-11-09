// Janelas: a "placa de vidro" (desfoque nativo do DWM recortado por região)
// e as janelas "layered" de conteúdo com alfa por pixel.
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::util::*;

pub const CLASS: &str = "GlassDockWnd";

// Etiquetas guardadas em GWLP_USERDATA: tipo << 16 | índice
pub const K_MAIN: u32 = 1;
pub const K_DOCK: u32 = 2;
pub const K_PLATE: u32 = 3;
pub const K_TRIGGER: u32 = 4;
pub const K_PANEL: u32 = 5;
pub const K_PANEL_PLATE: u32 = 6;
pub const K_PREVIEW: u32 = 7;
pub const K_PREVIEW_PLATE: u32 = 8;
pub const K_SETTINGS: u32 = 9;
pub const K_SETTINGS_PLATE: u32 = 10;
pub const K_TOAST: u32 = 11;
pub const K_EYEDROP: u32 = 12;
pub const K_MAGNIFIER: u32 = 13;
pub const K_PET: u32 = 14;

pub fn tag(kind: u32, idx: usize) -> isize {
    ((kind << 16) | (idx as u32 & 0xFFFF)) as isize
}

pub fn tag_kind(t: isize) -> u32 {
    (t as u32) >> 16
}

pub fn tag_idx(t: isize) -> usize {
    (t as u32 & 0xFFFF) as usize
}

fn hinst() -> HINSTANCE {
    unsafe { GetModuleHandleW(None).unwrap_or_default().into() }
}

pub fn create(kind: u32, idx: usize, ex: WINDOW_EX_STYLE, style: WINDOW_STYLE, owner: Option<HWND>, title: &str) -> HWND {
    unsafe {
        let cls = wide(CLASS);
        let t = wide(title);
        let h = CreateWindowExW(
            ex,
            pcw(&cls),
            pcw(&t),
            style,
            0,
            0,
            1,
            1,
            owner,
            None,
            Some(hinst()),
            None,
        )
        .unwrap_or_default();
        SetWindowLongPtrW(h, GWLP_USERDATA, tag(kind, idx));
        h
    }
}

/// Janela de conteúdo com alfa por pixel (UpdateLayeredWindow).
pub fn create_layered(kind: u32, idx: usize, owner: Option<HWND>, activatable: bool) -> HWND {
    let mut ex = WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST;
    if !activatable {
        ex |= WS_EX_NOACTIVATE;
    }
    create(kind, idx, ex, WS_POPUP, owner, "GlassDock")
}

/// Placa de vidro: janela comum com desfoque do DWM e região arredondada.
pub fn create_plate(kind: u32, idx: usize, owner: Option<HWND>) -> HWND {
    let h = create(kind, idx, WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE, WS_POPUP, owner, "GlassDockPlate");
    enable_blur(h);
    unsafe {
        let pref = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            h,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as _,
            std::mem::size_of_val(&pref) as u32,
        );
        let policy = DWMNCRP_DISABLED;
        let _ = DwmSetWindowAttribute(h, DWMWA_NCRENDERING_POLICY, &policy as *const _ as _, 4);
        // A placa nunca aparece em Alt+Tab nem em capturas de "peek".
        let excluded: i32 = 1;
        let _ = DwmSetWindowAttribute(h, DWMWA_EXCLUDED_FROM_PEEK, &excluded as *const _ as _, 4);
    }
    h
}

#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    color: u32,
    anim: u32,
}

#[repr(C)]
struct WinCompAttrData {
    attrib: u32,
    data: *mut core::ffi::c_void,
    size: usize,
}

type SetWca = unsafe extern "system" fn(HWND, *mut WinCompAttrData) -> BOOL;

pub fn enable_blur(h: HWND) {
    unsafe {
        let Ok(user32) = GetModuleHandleW(w!("user32.dll")) else { return };
        let Some(f) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else { return };
        let f: SetWca = std::mem::transmute(f);
        // ACCENT_ENABLE_BLURBEHIND = 3: desfoque gaussiano puro, a tinta é desenhada por nós.
        let mut ap = AccentPolicy { state: 3, flags: 0, color: 0, anim: 0 };
        let mut d = WinCompAttrData { attrib: 19, data: &mut ap as *mut _ as _, size: std::mem::size_of::<AccentPolicy>() };
        let _ = f(h, &mut d);
    }
}

/// Posiciona a placa (coordenadas de tela, pixels) e recorta com cantos arredondados.
pub fn place_plate(h: HWND, x: i32, y: i32, w: i32, hgt: i32, radius: i32, insert_after: Option<HWND>) {
    unsafe {
        let w = w.max(1);
        let hgt = hgt.max(1);
        let rgn = CreateRoundRectRgn(0, 0, w + 1, hgt + 1, radius * 2, radius * 2);
        let flags = SWP_NOACTIVATE | if insert_after.is_none() { SWP_NOZORDER } else { SET_WINDOW_POS_FLAGS(0) };
        let _ = SetWindowPos(h, insert_after, x, y, w, hgt, flags);
        SetWindowRgn(h, Some(rgn), true);
    }
}

pub fn show(h: HWND, visible: bool) {
    unsafe {
        let _ = ShowWindow(h, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
    }
}

pub fn is_visible(h: HWND) -> bool {
    unsafe { IsWindowVisible(h).as_bool() }
}

pub fn destroy(h: HWND) {
    unsafe {
        if !h.0.is_null() {
            let _ = DestroyWindow(h);
        }
    }
}

pub fn set_topmost(h: HWND) {
    unsafe {
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

/// Janela "gatilho" transparente que só recebe o mouse (borda da tela).
pub fn create_trigger(idx: usize) -> HWND {
    let h = create(
        K_TRIGGER,
        idx,
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
        WS_POPUP,
        None,
        "GlassDockEdge",
    );
    unsafe {
        let _ = SetLayeredWindowAttributes(h, COLORREF(0), 1, LWA_ALPHA);
    }
    h
}

pub fn window_rect(h: HWND) -> RECT {
    let mut r = RECT::default();
    unsafe {
        let _ = GetWindowRect(h, &mut r);
    }
    r
}

pub fn cursor_pos() -> POINT {
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    p
}

pub fn monitor_info(m: HMONITOR) -> (RECT, RECT) {
    unsafe {
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(m, &mut mi);
        (mi.rcMonitor, mi.rcWork)
    }
}

pub fn monitor_dpi(m: HMONITOR) -> u32 {
    unsafe {
        let (mut x, mut y) = (96u32, 96u32);
        let _ = windows::Win32::UI::HiDpi::GetDpiForMonitor(m, windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI, &mut x, &mut y);
        x.max(96)
    }
}

pub fn monitors() -> Vec<HMONITOR> {
    unsafe extern "system" fn cb(m: HMONITOR, _: HDC, _: *mut RECT, l: LPARAM) -> BOOL {
        let v = &mut *(l.0 as *mut Vec<HMONITOR>);
        v.push(m);
        TRUE
    }
    let mut v: Vec<HMONITOR> = vec![];
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut v as *mut _ as isize));
    }
    // monitor principal primeiro
    v.sort_by_key(|m| {
        let (r, _) = monitor_info(*m);
        if r.left == 0 && r.top == 0 {
            0
        } else {
            1
        }
    });
    v
}

pub fn primary_monitor() -> HMONITOR {
    unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) }
}

/// Sistema em modo escuro para apps?
pub fn system_dark() -> bool {
    use windows::Win32::System::Registry::*;
    unsafe {
        let mut v: u32 = 1;
        let mut sz = 4u32;
        let k = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let n = wide("AppsUseLightTheme");
        let r = RegGetValueW(HKEY_CURRENT_USER, pcw(&k), pcw(&n), RRF_RT_REG_DWORD, None, Some(&mut v as *mut _ as _), Some(&mut sz));
        r.is_ok() && v == 0
    }
}

/// Menus nativos no tema escuro (uxtheme, ordinais não documentados).
pub fn enable_dark_menus(dark: bool) {
    unsafe {
        let Ok(ux) = LoadLibraryW(w!("uxtheme.dll")) else { return };
        // SetPreferredAppMode = ordinal 135, FlushMenuThemes = 136
        if let Some(f) = GetProcAddress(ux, PCSTR(135usize as *const u8)) {
            let f: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(f);
            f(if dark { 2 } else { 3 });
        }
        if let Some(f) = GetProcAddress(ux, PCSTR(136usize as *const u8)) {
            let f: unsafe extern "system" fn() = std::mem::transmute(f);
            f();
        }
    }
}
