// Janelas: a "placa de vidro" (desfoque nativo do DWM recortado por região)
// e as janelas "layered" de conteúdo com alfa por pixel.
use windows::core::*;
use windows::core::Interface;
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

// ---------------------------------------------------------------------------
// Placa de vidro
//
// Preferência: Windows.UI.Composition com o "host backdrop" (o mesmo fundo
// desfocado do acrílico do sistema) num visual recortado por um retângulo
// arredondado com antisserrilhado. A janela não tem superfície GDI
// (WS_EX_NOREDIRECTIONBITMAP), então nada fora do recorte pode aparecer preto.
// Se a composição não estiver disponível, cai para o desfoque antigo.
// ---------------------------------------------------------------------------

struct PlateVis {
    _target: windows::UI::Composition::Desktop::DesktopWindowTarget,
    sprite: windows::UI::Composition::SpriteVisual,
    geom: windows::UI::Composition::CompositionRoundedRectangleGeometry,
}

struct Comp {
    _dq: windows::System::DispatcherQueueController,
    comp: windows::UI::Composition::Compositor,
    plates: std::collections::HashMap<isize, PlateVis>,
    failed: bool,
}

thread_local! {
    static COMP: std::cell::RefCell<Option<Comp>> = const { std::cell::RefCell::new(None) };
    static COMP_TRIED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn comp_init() -> Option<Comp> {
    use windows::Win32::System::WinRT::*;
    unsafe {
        // carregado na hora: se a DLL não existir, só não usamos a composição
        type CreateDq = unsafe extern "system" fn(DispatcherQueueOptions, *mut *mut core::ffi::c_void) -> HRESULT;
        let lib = LoadLibraryW(w!("CoreMessaging.dll")).ok()?;
        let f = GetProcAddress(lib, s!("CreateDispatcherQueueController"))?;
        let f: CreateDq = std::mem::transmute(f);
        let opts = DispatcherQueueOptions { dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32, threadType: DQTYPE_THREAD_CURRENT, apartmentType: DQTAT_COM_NONE };
        let mut raw: *mut core::ffi::c_void = std::ptr::null_mut();
        if f(opts, &mut raw).is_err() || raw.is_null() {
            return None;
        }
        let dq = windows::System::DispatcherQueueController::from_raw(raw);
        let comp = windows::UI::Composition::Compositor::new().ok()?;
        Some(Comp { _dq: dq, comp, plates: Default::default(), failed: false })
    }
}

/// O usuário desligou os efeitos de transparência do Windows?
pub fn transparency_enabled() -> bool {
    use windows::Win32::System::Registry::*;
    unsafe {
        let mut v: u32 = 1;
        let mut sz = 4u32;
        let k = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let n = wide("EnableTransparency");
        let r = RegGetValueW(HKEY_CURRENT_USER, pcw(&k), pcw(&n), RRF_RT_REG_DWORD, None, Some(&mut v as *mut _ as _), Some(&mut sz));
        !(r.is_ok() && v == 0)
    }
}

fn comp_attach(h: HWND) -> bool {
    use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
    if !COMP_TRIED.with(|t| t.replace(true)) {
        let c = comp_init();
        COMP.with(|x| *x.borrow_mut() = c);
    }
    COMP.with(|x| {
        let mut b = x.borrow_mut();
        let Some(c) = b.as_mut() else { return false };
        if c.failed {
            return false;
        }
        let r = (|| -> windows::core::Result<PlateVis> {
            let interop: ICompositorDesktopInterop = c.comp.cast()?;
            let target = unsafe { interop.CreateDesktopWindowTarget(h, false)? };
            let sprite = c.comp.CreateSpriteVisual()?;
            if transparency_enabled() {
                unsafe {
                    let on: i32 = 1;
                    // DWMWA_USE_HOSTBACKDROPBRUSH = 17
                    let _ = DwmSetWindowAttribute(h, DWMWINDOWATTRIBUTE(17), &on as *const _ as _, 4);
                }
                let brush = c.comp.CreateHostBackdropBrush()?;
                sprite.SetBrush(&brush)?;
            } else {
                // sem transparência no sistema: vidro sólido (nunca preto puro)
                let col = if system_dark() { windows::UI::Color { A: 255, R: 44, G: 44, B: 50 } } else { windows::UI::Color { A: 255, R: 236, G: 236, B: 242 } };
                let brush = c.comp.CreateColorBrushWithColor(col)?;
                sprite.SetBrush(&brush)?;
            }
            let geom = c.comp.CreateRoundedRectangleGeometry()?;
            let clip = c.comp.CreateGeometricClipWithGeometry(&geom)?;
            sprite.SetClip(&clip)?;
            target.SetRoot(&sprite)?;
            Ok(PlateVis { _target: target, sprite, geom })
        })();
        match r {
            Ok(pv) => {
                c.plates.insert(h.0 as isize, pv);
                true
            }
            Err(_) => {
                c.failed = true;
                false
            }
        }
    })
}

thread_local! {
    static GLASS_MODE: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
    /// janelas cuja placa usa o desfoque "accent" (precisam de recuo nos cantos)
    static ACCENT_PLATES: std::cell::RefCell<std::collections::HashSet<isize>> = std::cell::RefCell::new(Default::default());
}

/// 0 composição, 1 acrílico, 2 desfoque clássico, 3 transparente
pub fn set_glass_mode(m: u32) {
    GLASS_MODE.with(|g| g.set(m.min(3)));
}

pub fn glass_mode() -> u32 {
    GLASS_MODE.with(|g| g.get())
}

/// Sem desfoque nenhum (os painéis usam uma tinta mais forte).
pub fn transparent_mode() -> bool {
    glass_mode() == 3
}

fn plate_flags() -> WINDOW_EX_STYLE {
    WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE
}

fn plate_attrs(h: HWND, round: bool) {
    unsafe {
        let pref = if round { DWMWCP_ROUND } else { DWMWCP_DONOTROUND };
        let _ = DwmSetWindowAttribute(h, DWMWA_WINDOW_CORNER_PREFERENCE, &pref as *const _ as _, std::mem::size_of_val(&pref) as u32);
        let excluded: i32 = 1;
        let _ = DwmSetWindowAttribute(h, DWMWA_EXCLUDED_FROM_PEEK, &excluded as *const _ as _, 4);
    }
}

/// Placa de vidro atrás de uma janela de conteúdo.
pub fn create_plate(kind: u32, idx: usize, owner: Option<HWND>) -> HWND {
    let mut mode = glass_mode();
    // as miniaturas das prévias precisam de uma placa de verdade
    if mode == 3 && kind == K_PREVIEW_PLATE {
        mode = 1;
    }
    if mode == 3 {
        // transparente: placa invisível e que não recebe cliques
        let h = create(kind, idx, plate_flags() | WS_EX_LAYERED | WS_EX_TRANSPARENT, WS_POPUP, owner, "GlassDockPlate");
        unsafe {
            let _ = SetLayeredWindowAttributes(h, COLORREF(0), 0, LWA_ALPHA);
        }
        return h;
    }
    if mode == 0 {
        let h = create(kind, idx, plate_flags() | WS_EX_NOREDIRECTIONBITMAP, WS_POPUP, owner, "GlassDockPlate");
        plate_attrs(h, false);
        unsafe {
            let policy = DWMNCRP_DISABLED;
            let _ = DwmSetWindowAttribute(h, DWMWA_NCRENDERING_POLICY, &policy as *const _ as _, 4);
        }
        if comp_attach(h) {
            return h;
        }
        destroy(h);
        mode = 1;
    }
    // acrílico / desfoque clássico pelo "accent" do DWM
    let h = create(kind, idx, plate_flags(), WS_POPUP, owner, "GlassDockPlate");
    plate_attrs(h, true);
    enable_accent(h, if mode == 2 { 3 } else { 4 });
    ACCENT_PLATES.with(|a| a.borrow_mut().insert(h.0 as isize));
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
    enable_accent(h, 3);
}

pub fn enable_accent(h: HWND, state: u32) {
    unsafe {
        let Ok(user32) = GetModuleHandleW(w!("user32.dll")) else { return };
        let Some(f) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else { return };
        let f: SetWca = std::mem::transmute(f);
        // ACCENT_ENABLE_BLURBEHIND = 3: desfoque gaussiano puro, a tinta é desenhada por nós.
        // acrílico precisa de alguma cor (AABBGGRR); quase transparente = mais vidro
        let color = if state == 4 { 0x10_14_12_12 } else { 0 };
        let mut ap = AccentPolicy { state, flags: if state == 4 { 2 } else { 0 }, color, anim: 0 };
        let mut d = WinCompAttrData { attrib: 19, data: &mut ap as *mut _ as _, size: std::mem::size_of::<AccentPolicy>() };
        let _ = f(h, &mut d);
    }
}

/// Posiciona a placa (coordenadas de tela, pixels) com cantos arredondados.
pub fn place_plate(h: HWND, x: i32, y: i32, w: i32, hgt: i32, radius: i32, insert_after: Option<HWND>) {
    unsafe {
        let w = w.max(1);
        let hgt = hgt.max(1);
        let flags = SWP_NOACTIVATE | if insert_after.is_none() { SWP_NOZORDER } else { SET_WINDOW_POS_FLAGS(0) };
        let _ = SetWindowPos(h, insert_after, x, y, w, hgt, flags);
        let done = COMP.with(|x| {
            let b = x.borrow();
            let Some(pv) = b.as_ref().and_then(|c| c.plates.get(&(h.0 as isize))) else { return false };
            let size = windows_numerics::Vector2 { X: w as f32, Y: hgt as f32 };
            let r = (radius as f32).min(w as f32 / 2.0).min(hgt as f32 / 2.0);
            let _ = pv.sprite.SetSize(size);
            let _ = pv.geom.SetSize(size);
            let _ = pv.geom.SetCornerRadius(windows_numerics::Vector2 { X: r, Y: r });
            true
        });
        if !done {
            let accent = ACCENT_PLATES.with(|a| a.borrow().contains(&(h.0 as isize)));
            if accent {
                // O desfoque "accent" ignora a região da janela; o Windows 11
                // só arredonda 8 px. Encolhe a placa até ela caber inteira
                // dentro da forma arredondada (as pontas ficam só com a tinta).
                let dpi = windows::Win32::UI::HiDpi::GetDpiForWindow(h).max(96) as f32;
                let c = 8.0 * dpi / 96.0 + 1.0;
                let r = (radius as f32).min(w as f32 / 2.0).min(hgt as f32 / 2.0);
                let (ix, iy) = if r <= c {
                    (0.0, 0.0)
                } else {
                    let t = (r * 0.12).max(1.0);
                    let rc = r - c;
                    let i = rc - (rc * rc - (rc - t).max(0.0).powi(2)).max(0.0).sqrt();
                    (i.ceil(), t.ceil())
                };
                let _ = SetWindowPos(h, None, x + ix as i32, y + iy as i32, (w - 2 * ix as i32).max(1), (hgt - 2 * iy as i32).max(1), SWP_NOACTIVATE | SWP_NOZORDER);
            }
            let rgn = CreateRoundRectRgn(0, 0, w + 1, hgt + 1, radius * 2, radius * 2);
            if !accent {
                SetWindowRgn(h, Some(rgn), true);
            } else {
                let _ = DeleteObject(rgn.into());
            }
        }
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
    ACCENT_PLATES.with(|a| {
        if let Ok(mut b) = a.try_borrow_mut() {
            b.remove(&(h.0 as isize));
        }
    });
    COMP.with(|x| {
        if let Ok(mut b) = x.try_borrow_mut() {
            if let Some(c) = b.as_mut() {
                c.plates.remove(&(h.0 as isize));
            }
        }
    });
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

/// Há um app em tela cheia de verdade na frente deste monitor? (usado para
/// destravar a dock quando o aviso de "saiu da tela cheia" do Windows se perde)
pub fn fullscreen_on(m: HMONITOR) -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(fg, Some(&mut pid));
        if pid == windows::Win32::System::Threading::GetCurrentProcessId() {
            return false;
        }
        let mut cls = [0u16; 64];
        let n = GetClassNameW(fg, &mut cls).max(0) as usize;
        let c = String::from_utf16_lossy(&cls[..n]);
        if matches!(c.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Windows.UI.Core.CoreWindow") {
            return false;
        }
        if MonitorFromWindow(fg, MONITOR_DEFAULTTONULL) != m {
            return false;
        }
        // janela maximizada comum (com barra de título) não é tela cheia
        let style = GetWindowLongW(fg, GWL_STYLE) as u32;
        if style & WS_CAPTION.0 == WS_CAPTION.0 {
            return false;
        }
        let mut r = RECT::default();
        let _ = GetWindowRect(fg, &mut r);
        let (mon, _) = monitor_info(m);
        r.left <= mon.left && r.top <= mon.top && r.right >= mon.right && r.bottom >= mon.bottom
    }
}
