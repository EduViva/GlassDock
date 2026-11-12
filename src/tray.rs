// Bandeja do sistema: o GlassDock se apresenta como "Shell_TrayWnd" para
// receber os ícones de notificação dos apps (e repassa tudo ao Explorer,
// que continua com os ícones e as notificações). Assim os ícones aparecem
// em linha na dock, mesmo com a barra de tarefas escondida.
use std::cell::RefCell;
use std::collections::HashMap;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::util::*;

#[repr(C)]
#[derive(Clone, Copy)]
struct Nid32 {
    cb_size: u32,
    hwnd: u32,
    uid: u32,
    flags: u32,
    callback: u32,
    hicon: u32,
    tip: [u16; 128],
    state: u32,
    state_mask: u32,
    info: [u16; 256],
    version: u32,
    info_title: [u16; 64],
    info_flags: u32,
    guid: GUID,
    balloon_icon: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TrayData {
    sig: u32,
    msg: u32,
    nid: Nid32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct IconIdent32 {
    magic: u32,
    msg: u32,
    cb: u32,
    pad: u32,
    hwnd: u32,
    uid: u32,
    guid: GUID,
}

const NIF_MESSAGE: u32 = 1;
const NIF_ICON: u32 = 2;
const NIF_TIP: u32 = 4;
const NIF_STATE: u32 = 8;
const NIF_GUID: u32 = 0x20;
const NIS_HIDDEN: u32 = 1;

#[derive(Clone, Default)]
pub struct TrayIcon {
    pub hwnd: HWND,
    pub uid: u32,
    pub guid: Option<GUID>,
    pub callback: u32,
    pub hicon: HICON,
    pub tip: String,
    pub version: u32,
    pub hidden: bool,
    pub exe: String,
    pub pid: u32,
    pub key: String,
    pub serial: u64,
}

#[derive(Default)]
pub struct TrayStore {
    pub icons: Vec<TrayIcon>,
    pub rects: HashMap<String, RECT>,
    pub explorer: HWND,
    pub own: HWND,
    serial: u64,
    /// selos (overlay) que os apps põem no botão da barra: janela -> (cópia do ícone, série)
    pub overlays: HashMap<isize, (isize, u64)>,
    /// nossa janela que recebe as chamadas do ITaskbarList e a original do Explorer
    band: HWND,
    real_band: HWND,
}

thread_local! {
    pub static TRAY: RefCell<TrayStore> = RefCell::new(TrayStore::default());
}

pub fn with<R>(f: impl FnOnce(&mut TrayStore) -> R) -> R {
    TRAY.with(|t| f(&mut t.borrow_mut()))
}

fn icon_key(exe: &str, uid: u32, guid: Option<GUID>) -> String {
    let base = std::path::Path::new(exe).file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    match guid {
        Some(g) => format!("{}|{:?}", base, g),
        None => format!("{}|{}", base, uid),
    }
}

pub fn start(explorer: HWND) {
    unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let cls = w!("Shell_TrayWnd");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(tray_proc),
            hInstance: hinst,
            lpszClassName: cls,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        let h = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            cls,
            w!(""),
            WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinst),
            None,
        )
        .unwrap_or_default();
        with(|t| {
            t.own = h;
            t.explorer = explorer;
        });
        // alguns apps procuram a área de notificação dentro da barra
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("Static"),
            w!(""),
            WS_CHILD,
            0,
            0,
            0,
            0,
            Some(h),
            None,
            Some(hinst),
            None,
        );
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        // ITaskbarList (selos de notificação, progresso…) procura esta propriedade
        // na Shell_TrayWnd: apontamos para uma janela nossa, que repassa ao Explorer
        let bcls = w!("GlassDockTaskband");
        let wc2 = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(band_proc),
            hInstance: hinst,
            lpszClassName: bcls,
            ..Default::default()
        };
        RegisterClassExW(&wc2);
        let band = CreateWindowExW(WS_EX_TOOLWINDOW, bcls, w!(""), WS_POPUP, 0, 0, 0, 0, None, None, Some(hinst), None).unwrap_or_default();
        let real = HWND(GetPropW(explorer, w!("TaskbandHWND")).0);
        with(|t| {
            t.band = band;
            t.real_band = real;
        });
        if !band.0.is_null() {
            let _ = SetPropW(h, w!("TaskbandHWND"), Some(HANDLE(band.0)));
        }
        rebroadcast();
    }
}

/// Pede a todos os apps que recriem seus ícones (agora conosco).
pub fn rebroadcast() {
    unsafe {
        let msg = RegisterWindowMessageW(w!("TaskbarCreated"));
        let _ = SendNotifyMessageW(HWND_BROADCAST, msg, WPARAM(0), LPARAM(0));
    }
}

/// Mantém a nossa janela à frente da do Explorer (FindWindow pega a primeira).
pub fn ensure_first() -> bool {
    unsafe {
        let own = with(|t| t.own);
        if own.0.is_null() {
            return true;
        }
        let first = FindWindowW(w!("Shell_TrayWnd"), None).unwrap_or_default();
        if first != own {
            let _ = SetWindowPos(own, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            return false;
        }
        true
    }
}

pub fn set_explorer(h: HWND) {
    let real = unsafe { HWND(GetPropW(h, w!("TaskbandHWND")).0) };
    with(|t| {
        t.explorer = h;
        t.real_band = real;
    });
}

/// Selo (overlay) de uma janela, se o app pôs um: (ícone, série para o cache).
pub fn overlay_of(hwnd: HWND) -> Option<(HICON, u64)> {
    with(|t| t.overlays.get(&(hwnd.0 as isize)).map(|(i, s)| (HICON(*i as _), *s)))
}

/// Esquece os selos de janelas que fecharam.
pub fn prune_overlays() {
    let dead: Vec<isize> = with(|t| t.overlays.iter().filter(|(h, _)| unsafe { !IsWindow(Some(HWND(**h as _))).as_bool() }).map(|(h, (i, _))| {
        unsafe {
            let _ = DestroyIcon(HICON(*i as _));
        }
        *h
    }).collect());
    if !dead.is_empty() {
        with(|t| {
            for h in dead {
                t.overlays.remove(&h);
            }
        });
    }
}

/// Recebe as mensagens do ITaskbarList dos apps. 67 = SetOverlayIcon (wParam
/// janela, lParam ícone). Tudo é repassado à barra do Explorer, que continua
/// sabendo de tudo (e mostra certo se o GlassDock fechar).
unsafe extern "system" fn band_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_USER + 67 {
        let target = wp.0 as isize;
        let copy = if lp.0 != 0 { CopyIcon(HICON(lp.0 as _)).map(|i| i.0 as isize).unwrap_or(0) } else { 0 };
        let old = with(|t| {
            t.serial += 1;
            let ser = t.serial;
            if copy != 0 {
                t.overlays.insert(target, (copy, ser)).map(|x| x.0)
            } else {
                t.overlays.remove(&target).map(|x| x.0)
            }
        });
        if let Some(o) = old {
            let _ = DestroyIcon(HICON(o as _));
        }
        crate::app::post(crate::app::WM_APP_OVERLAY, 0, 0);
    }
    if msg >= WM_USER {
        let real = with(|t| t.real_band);
        if !real.0.is_null() && IsWindow(Some(real)).as_bool() {
            if InSendMessage().as_bool() {
                let mut res: usize = 0;
                let _ = SendMessageTimeoutW(real, msg, wp, lp, SMTO_ABORTIFHUNG, 1000, Some(&mut res));
                return LRESULT(res as isize);
            }
            let _ = PostMessageW(Some(real), msg, wp, lp);
        }
        return LRESULT(0);
    }
    DefWindowProcW(h, msg, wp, lp)
}

pub fn stop() {
    let own = with(|t| std::mem::take(&mut t.own));
    if !own.0.is_null() {
        unsafe {
            let _ = RemovePropW(own, w!("TaskbandHWND"));
            let _ = DestroyWindow(own);
        }
    }
    let band = with(|t| std::mem::take(&mut t.band));
    if !band.0.is_null() {
        unsafe {
            let _ = DestroyWindow(band);
        }
    }
    with(|t| {
        for i in t.icons.drain(..) {
            unsafe {
                let _ = DestroyIcon(i.hicon);
            }
        }
    });
}

unsafe fn forward(msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let ex = with(|t| t.explorer);
    if ex.0.is_null() || !IsWindow(Some(ex)).as_bool() {
        return LRESULT(0);
    }
    let mut res: usize = 0;
    let _ = SendMessageTimeoutW(ex, msg, wp, lp, SMTO_ABORTIFHUNG, 2000, Some(&mut res));
    LRESULT(res as isize)
}

unsafe extern "system" fn tray_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_COPYDATA {
        let cds = &*(lp.0 as *const COPYDATASTRUCT);
        match cds.dwData {
            1 => {
                let mut td: TrayData = std::mem::zeroed();
                let n = (cds.cbData as usize).min(std::mem::size_of::<TrayData>());
                std::ptr::copy_nonoverlapping(cds.lpData as *const u8, &mut td as *mut _ as *mut u8, n);
                let ours = if td.sig == 0x34753423 { handle_notify(&td) } else { false };
                let _ = forward(msg, wp, lp);
                if ours {
                    crate::app::post(crate::app::WM_APP_TRAY, 0, 0);
                }
                return LRESULT(ours as isize);
            }
            3 => {
                let mut id: IconIdent32 = std::mem::zeroed();
                let n = (cds.cbData as usize).min(std::mem::size_of::<IconIdent32>());
                std::ptr::copy_nonoverlapping(cds.lpData as *const u8, &mut id as *mut _ as *mut u8, n);
                let guid = if id.guid != GUID::zeroed() { Some(id.guid) } else { None };
                let rect = with(|t| {
                    let ic = t.icons.iter().find(|i| match guid {
                        Some(g) => i.guid == Some(g),
                        None => i.hwnd.0 as usize as u32 == id.hwnd && i.uid == id.uid,
                    })?;
                    t.rects.get(&ic.key).copied()
                });
                if let Some(r) = rect {
                    let v = if id.msg == 1 { (r.left, r.top) } else { (r.right, r.bottom) };
                    return LRESULT(((v.0 as u16 as u32) | ((v.1 as u16 as u32) << 16)) as i32 as isize);
                }
                return forward(msg, wp, lp);
            }
            _ => return forward(msg, wp, lp),
        }
    }
    if msg == WM_WINDOWPOSCHANGING {
        return LRESULT(0);
    }
    // mensagens privadas do Shell (Iniciar, notificações…) seguem para a barra real
    if msg >= WM_USER || msg == WM_COMMAND || msg == WM_SYSCOMMAND || msg == WM_HOTKEY {
        let ex = with(|t| t.explorer);
        if !ex.0.is_null() {
            if InSendMessage().as_bool() {
                return forward(msg, wp, lp);
            }
            let _ = PostMessageW(Some(ex), msg, wp, lp);
            return LRESULT(0);
        }
    }
    DefWindowProcW(h, msg, wp, lp)
}

fn handle_notify(td: &TrayData) -> bool {
    let nid = &td.nid;
    let hwnd = HWND(nid.hwnd as usize as _);
    let guid = if nid.flags & NIF_GUID != 0 && nid.guid != GUID::zeroed() { Some(nid.guid) } else { None };
    with(|t| {
        let pos = t.icons.iter().position(|i| match guid {
            Some(g) => i.guid == Some(g),
            None => i.hwnd == hwnd && i.uid == nid.uid,
        });
        match td.msg {
            0 | 1 => {
                // NIM_ADD / NIM_MODIFY
                if td.msg == 0 && pos.is_some() {
                    return false;
                }
                if td.msg == 1 && pos.is_none() {
                    return false;
                }
                let idx = match pos {
                    Some(i) => i,
                    None => {
                        let mut pid = 0u32;
                        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
                        let exe = process_image_path(pid).unwrap_or_default();
                        let key = icon_key(&exe, nid.uid, guid);
                        t.icons.push(TrayIcon {
                            hwnd,
                            uid: nid.uid,
                            guid,
                            callback: 0,
                            hicon: HICON::default(),
                            tip: String::new(),
                            version: 0,
                            hidden: false,
                            exe,
                            pid,
                            key,
                            serial: 0,
                        });
                        t.icons.len() - 1
                    }
                };
                t.serial += 1;
                let serial = t.serial;
                let ic = &mut t.icons[idx];
                ic.serial = serial;
                if nid.flags & NIF_MESSAGE != 0 {
                    ic.callback = nid.callback;
                }
                if nid.flags & NIF_ICON != 0 {
                    unsafe {
                        if !ic.hicon.is_invalid() {
                            let _ = DestroyIcon(ic.hicon);
                        }
                        ic.hicon = CopyIcon(HICON(nid.hicon as usize as _)).unwrap_or_default();
                    }
                }
                if nid.flags & NIF_TIP != 0 {
                    ic.tip = from_wide(&nid.tip);
                }
                if nid.flags & NIF_STATE != 0 && nid.state_mask & NIS_HIDDEN != 0 {
                    ic.hidden = nid.state & NIS_HIDDEN != 0;
                }
                true
            }
            2 => {
                // NIM_DELETE
                if let Some(i) = pos {
                    let ic = t.icons.remove(i);
                    unsafe {
                        let _ = DestroyIcon(ic.hicon);
                    }
                    true
                } else {
                    false
                }
            }
            4 => {
                // NIM_SETVERSION
                if let Some(i) = pos {
                    t.icons[i].version = nid.version;
                    true
                } else {
                    false
                }
            }
            _ => pos.is_some(),
        }
    })
}

/// Remove ícones de janelas que não existem mais.
pub fn prune() -> bool {
    with(|t| {
        let before = t.icons.len();
        t.icons.retain(|i| unsafe { IsWindow(Some(i.hwnd)).as_bool() });
        before != t.icons.len()
    })
}

pub fn visible_icons() -> Vec<TrayIcon> {
    with(|t| t.icons.iter().filter(|i| !i.hidden && !i.hicon.is_invalid()).cloned().collect())
}

pub fn set_rect(key: &str, r: RECT) {
    with(|t| {
        t.rects.insert(key.to_string(), r);
    });
}

#[derive(Clone, Copy, PartialEq)]
pub enum Click {
    Left,
    Right,
    Double,
    Middle,
}

/// Repassa um clique ao app dono do ícone, do jeito que a barra faria.
pub fn click(key: &str, kind: Click, x: i32, y: i32) {
    let Some(ic) = with(|t| t.icons.iter().find(|i| i.key == key).cloned()) else { return };
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(ic.pid);
        let send = |m: u32| {
            let (wp, lp) = if ic.version >= 4 {
                (
                    WPARAM(((x as u16 as u32) | ((y as u16 as u32) << 16)) as usize),
                    LPARAM(((m & 0xFFFF) | ((ic.uid & 0xFFFF) << 16)) as isize),
                )
            } else {
                (WPARAM(ic.uid as usize), LPARAM(m as isize))
            };
            let _ = PostMessageW(Some(ic.hwnd), ic.callback, wp, lp);
        };
        const NIN_SELECT: u32 = WM_USER;
        match kind {
            Click::Left => {
                send(WM_LBUTTONDOWN);
                send(WM_LBUTTONUP);
                if ic.version >= 3 {
                    send(NIN_SELECT);
                }
            }
            Click::Double => {
                send(WM_LBUTTONDBLCLK);
                send(WM_LBUTTONUP);
            }
            Click::Right => {
                send(WM_RBUTTONDOWN);
                send(WM_RBUTTONUP);
                if ic.version >= 3 {
                    send(WM_CONTEXTMENU);
                }
            }
            Click::Middle => {
                send(WM_MBUTTONDOWN);
                send(WM_MBUTTONUP);
            }
        }
    }
}

/// Ícones que o próprio Windows mostra na barra (promovidos), para começar igual.
pub fn windows_promoted_exes() -> Vec<String> {
    use windows::Win32::System::Registry::*;
    let mut out = vec![];
    unsafe {
        let base = wide("Control Panel\\NotifyIconSettings");
        let mut hk = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, pcw(&base), None, KEY_READ, &mut hk).is_err() {
            return out;
        }
        let mut i = 0u32;
        loop {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            if RegEnumKeyExW(hk, i, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None).is_err() {
                break;
            }
            i += 1;
            let sub = from_wide(&name);
            let mut promoted: u32 = 0;
            let mut sz = 4u32;
            let subw = wide(&sub);
            let v = wide("IsPromoted");
            if RegGetValueW(hk, pcw(&subw), pcw(&v), RRF_RT_REG_DWORD, None, Some(&mut promoted as *mut _ as _), Some(&mut sz)).is_ok()
                && promoted == 1
            {
                let mut buf = [0u16; 1024];
                let mut bsz = (buf.len() * 2) as u32;
                let p = wide("ExecutablePath");
                if RegGetValueW(hk, pcw(&subw), pcw(&p), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as _), Some(&mut bsz)).is_ok() {
                    let path = from_wide(&buf);
                    if let Some(f) = std::path::Path::new(&path).file_name() {
                        out.push(f.to_string_lossy().to_lowercase());
                    }
                }
            }
        }
        let _ = RegCloseKey(hk);
    }
    out
}
