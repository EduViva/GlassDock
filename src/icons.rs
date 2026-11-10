// Ícones de apps (pelo Shell, em alta resolução) com cache, catálogo de apps
// instalados e os fixados atuais da barra de tarefas.
use std::collections::HashMap;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::EnhancedStorage::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::UI::Shell::PropertiesSystem::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::PinnedApp;
use crate::gfx::gfx;
use crate::util::*;

#[derive(Clone)]
pub struct Icon {
    pub bmp: ID2D1Bitmap,
    /// O ícone ocupa o quadrado inteiro (não precisa de placa)
    pub full: bool,
}

#[derive(Default)]
pub struct IconCache {
    map: HashMap<(String, u32), Option<Icon>>,
}

impl IconCache {
    /// `path` pode ser um arquivo, pasta, .lnk ou "shell:AppsFolder\AUMID".
    pub fn get(&mut self, path: &str, px: u32) -> Option<Icon> {
        let px = px.max(1).next_power_of_two().clamp(32, 256);
        let key = (path.to_lowercase(), px);
        if let Some(v) = self.map.get(&key) {
            return v.clone();
        }
        let v = load_shell_icon(path, px as i32, true);
        self.map.insert(key, v.clone());
        v
    }

    /// Como `get`, mas com uma chave própria (o mesmo caminho em estados diferentes).
    pub fn get_keyed(&mut self, key: &str, path: &str, px: u32) -> Option<Icon> {
        let px = px.max(1).next_power_of_two().clamp(32, 256);
        let k = (format!("key:{}", key), px);
        if let Some(v) = self.map.get(&k) {
            return v.clone();
        }
        let v = load_shell_icon(path, px as i32, true);
        self.map.insert(k, v.clone());
        v
    }

    /// Miniatura (imagens/vídeos) em vez de ícone.
    pub fn thumb(&mut self, path: &str, px: u32) -> Option<Icon> {
        let key = (format!("thumb:{}", path.to_lowercase()), px);
        if let Some(v) = self.map.get(&key) {
            return v.clone();
        }
        let v = load_shell_icon(path, px as i32, false);
        self.map.insert(key, v.clone());
        v
    }

    pub fn put_hicon(&mut self, key: &str, hicon: HICON) -> Option<Icon> {
        let k = (key.to_lowercase(), 0);
        let bmp = gfx().bitmap_from_hicon(hicon)?;
        let ic = Icon { bmp, full: false };
        self.map.insert(k, Some(ic.clone()));
        Some(ic)
    }

    pub fn get_hicon_cached(&self, key: &str) -> Option<Icon> {
        self.map.get(&(key.to_lowercase(), 0)).cloned().flatten()
    }

    pub fn forget(&mut self, path: &str) {
        let p = path.to_lowercase();
        self.map.retain(|k, _| k.0 != p && k.0 != format!("thumb:{}", p));
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}

fn load_shell_icon(path: &str, px: i32, icon_only: bool) -> Option<Icon> {
    unsafe {
        let w = wide(path);
        let item: IShellItem = SHCreateItemFromParsingName(pcw(&w), None).ok()?;
        let fac: IShellItemImageFactory = item.cast().ok()?;
        let flags = if icon_only { SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK } else { SIIGBF_BIGGERSIZEOK };
        let hbm = fac.GetImage(SIZE { cx: px, cy: px }, flags).ok()?;
        let full = icon_only && fills_square(hbm);
        let bmp = gfx().bitmap_from_hbitmap(hbm);
        let _ = DeleteObject(hbm.into());
        Some(Icon { bmp: bmp?, full })
    }
}

/// Verifica se a parte opaca do ícone cobre praticamente o quadrado todo.
fn fills_square(hbm: HBITMAP) -> bool {
    unsafe {
        let mut bm = BITMAP::default();
        if GetObjectW(hbm.into(), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as _)) == 0 {
            return false;
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
        if w <= 0 || h <= 0 || bm.bmBitsPixel != 32 || bm.bmBits.is_null() {
            return false;
        }
        let px = std::slice::from_raw_parts(bm.bmBits as *const u8, (w * h * 4) as usize);
        let (mut minx, mut miny, mut maxx, mut maxy) = (w, h, -1, -1);
        let mut opaque = 0usize;
        for y in 0..h {
            for x in 0..w {
                let a = px[((y * w + x) * 4 + 3) as usize];
                if a > 40 {
                    opaque += 1;
                    minx = minx.min(x);
                    maxx = maxx.max(x);
                    miny = miny.min(y);
                    maxy = maxy.max(y);
                }
            }
        }
        if maxx < 0 {
            return false;
        }
        let bw = (maxx - minx + 1) as f32 / w as f32;
        let bh = (maxy - miny + 1) as f32 / h as f32;
        let density = opaque as f32 / (w * h) as f32;
        bw > 0.9 && bh > 0.9 && density > 0.78
    }
}

// ---------------------------------------------------------------------------
// Catálogo de apps instalados (shell:AppsFolder)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct CatalogApp {
    pub name: String,
    pub parsing: String,
    pub exe: String,
}

impl CatalogApp {
    pub fn to_pinned(&self) -> PinnedApp {
        let (aumid, exe) = resolve_parsing(&self.parsing, &self.exe);
        PinnedApp {
            name: self.name.clone(),
            launch: format!("shell:AppsFolder\\{}", self.parsing),
            args: String::new(),
            exe,
            aumid,
            ..Default::default()
        }
    }
}

/// Converte o "parsing name" do AppsFolder em (AUMID, exe).
fn resolve_parsing(parsing: &str, exe_hint: &str) -> (String, String) {
    // Apps desktop sem AUMID explícito: "{GUID-de-pasta-conhecida}\caminho\app.exe"
    if parsing.starts_with('{') {
        if let Some(end) = parsing.find('}') {
            let guid_s = &parsing[..=end];
            let rest = parsing[end + 1..].trim_start_matches('\\');
            let w = wide(guid_s);
            if let Ok(g) = unsafe { CLSIDFromString(pcw(&w)) } {
                if let Some(base) = known_folder(&g) {
                    let full = base.join(rest).to_string_lossy().to_string();
                    return (String::new(), full);
                }
            }
        }
    }
    if parsing.contains(":\\") {
        return (String::new(), parsing.to_string());
    }
    (parsing.to_string(), exe_hint.to_string())
}

pub fn catalog() -> Vec<CatalogApp> {
    let mut out = vec![];
    unsafe {
        let w = wide("shell:AppsFolder");
        let Ok(folder) = SHCreateItemFromParsingName::<_, _, IShellItem>(pcw(&w), None) else { return out };
        let Ok(en) = folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) else { return out };
        loop {
            let mut items = [None];
            let mut fetched = 0u32;
            if en.Next(&mut items, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = items[0].take() else { break };
            let name = item.GetDisplayName(SIGDN_NORMALDISPLAY).map(|p| from_pwstr(p)).unwrap_or_default();
            let parsing = item.GetDisplayName(SIGDN_PARENTRELATIVEPARSING).map(|p| from_pwstr(p)).unwrap_or_default();
            let mut exe = String::new();
            if let Ok(i2) = item.cast::<IShellItem2>() {
                if let Ok(p) = i2.GetString(&PKEY_Link_TargetParsingPath) {
                    exe = from_pwstr(p);
                }
            }
            if !name.is_empty() && !parsing.is_empty() {
                out.push(CatalogApp { name, parsing, exe });
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.parsing == b.parsing);
    out
}

/// Lê destino, argumentos e AUMID de um atalho .lnk.
pub fn resolve_lnk(path: &str) -> Option<(String, String, String)> {
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let pf: IPersistFile = link.cast().ok()?;
        let w = wide(path);
        pf.Load(pcw(&w), STGM_READ).ok()?;
        let mut buf = [0u16; 1024];
        let _ = link.GetPath(&mut buf, std::ptr::null_mut(), 0);
        let target = from_wide(&buf);
        let mut abuf = [0u16; 1024];
        let _ = link.GetArguments(&mut abuf);
        let args = from_wide(&abuf);
        let mut aumid = String::new();
        if let Ok(store) = link.cast::<IPropertyStore>() {
            if let Ok(pv) = store.GetValue(&PKEY_AppUserModel_ID) {
                if let Ok(p) = PropVariantToStringAlloc(&pv) {
                    aumid = from_pwstr(p);
                    CoTaskMemFree(Some(p.0 as _));
                }
            }
        }
        Some((target, args, aumid))
    }
}

pub fn pinned_from_path(path: &str) -> PinnedApp {
    let name = file_stem(path);
    if path.to_lowercase().ends_with(".lnk") {
        if let Some((target, args, aumid)) = resolve_lnk(path) {
            return PinnedApp { name, launch: path.to_string(), args: String::new(), exe: target, aumid, lnk_args: args, resolved: true, ..Default::default() };
        }
    }
    PinnedApp { name, launch: path.to_string(), args: String::new(), exe: path.to_string(), aumid: String::new(), resolved: true, ..Default::default() }
}

/// Fixados iniciais: os mesmos da barra de tarefas atual.
pub fn default_pins() -> Vec<PinnedApp> {
    let mut out = vec![];
    let win = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    out.push(PinnedApp {
        name: "Explorador de Arquivos".into(),
        launch: format!("{}\\explorer.exe", win),
        args: String::new(),
        exe: format!("{}\\explorer.exe", win),
        aumid: "Microsoft.Windows.Explorer".into(),
        ..Default::default()
    });
    let dir = std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
        .join("Microsoft\\Internet Explorer\\Quick Launch\\User Pinned\\TaskBar");
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut paths: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            let s = p.to_string_lossy().to_string();
            if !s.to_lowercase().ends_with(".lnk") {
                continue;
            }
            let app = pinned_from_path(&s);
            if app.exe.to_lowercase().ends_with("explorer.exe") {
                continue;
            }
            if !out.iter().any(|a| a.exe.eq_ignore_ascii_case(&app.exe) && !app.exe.is_empty()) {
                out.push(app);
            }
        }
    }
    if out.len() < 3 {
        for (name, aumid) in [
            ("Microsoft Edge", "MSEdge"),
            ("Configurações", "windows.immersivecontrolpanel_cw5n1h2txyewy!microsoft.windows.immersivecontrolpanel"),
        ] {
            out.push(PinnedApp {
                name: name.into(),
                launch: format!("shell:AppsFolder\\{}", aumid),
                args: String::new(),
                exe: if aumid == "MSEdge" { edge_path() } else { String::new() },
                aumid: aumid.into(),
                ..Default::default()
            });
        }
    }
    out
}

fn edge_path() -> String {
    for base in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(b) = std::env::var(base) {
            let p = format!("{}\\Microsoft\\Edge\\Application\\msedge.exe", b);
            if std::path::Path::new(&p).exists() {
                return p;
            }
        }
    }
    String::new()
}

/// Caminho usado para pegar o ícone de um app fixado.
pub fn icon_path(app: &PinnedApp) -> String {
    if !app.aumid.is_empty() && app.launch.starts_with("shell:AppsFolder") {
        return app.launch.clone();
    }
    if app.launch.to_lowercase().ends_with(".lnk") {
        return app.launch.clone();
    }
    if !app.exe.is_empty() {
        return app.exe.clone();
    }
    app.launch.clone()
}

/// Ícone de uma janela (WM_GETICON) como último recurso.
pub fn window_hicon(h: HWND) -> Option<HICON> {
    unsafe {
        let mut res: usize = 0;
        for kind in [ICON_BIG, ICON_SMALL2, ICON_SMALL] {
            if SendMessageTimeoutW(h, WM_GETICON, WPARAM(kind as usize), LPARAM(0), SMTO_ABORTIFHUNG, 100, Some(&mut res)).0 != 0
                && res != 0
            {
                return Some(HICON(res as _));
            }
        }
        let v = GetClassLongPtrW(h, GCLP_HICON);
        if v != 0 {
            return Some(HICON(v as _));
        }
        None
    }
}

thread_local! {
    static NAMES: std::cell::RefCell<HashMap<String, String>> = std::cell::RefCell::new(HashMap::new());
}

/// Nome amigável de um app (descrição do executável ou nome no Iniciar).
pub fn app_display_name(exe: &str, aumid: &str) -> String {
    let key = format!("{}|{}", exe.to_lowercase(), aumid.to_lowercase());
    if let Some(n) = NAMES.with(|m| m.borrow().get(&key).cloned()) {
        return n;
    }
    let mut name = String::new();
    if !aumid.is_empty() {
        unsafe {
            let w = wide(&format!("shell:AppsFolder\\{}", aumid));
            if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(pcw(&w), None) {
                if let Ok(p) = item.GetDisplayName(SIGDN_NORMALDISPLAY) {
                    name = from_pwstr(p);
                    CoTaskMemFree(Some(p.0 as _));
                }
            }
        }
    }
    if name.is_empty() && !exe.is_empty() {
        name = file_description(exe).unwrap_or_default();
    }
    if name.is_empty() {
        let st = file_stem(exe);
        let mut c = st.chars();
        name = match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => "App".into(),
        };
    }
    NAMES.with(|m| m.borrow_mut().insert(key, name.clone()));
    name
}

fn file_description(exe: &str) -> Option<String> {
    use windows::Win32::Storage::FileSystem::*;
    unsafe {
        let w = wide(exe);
        let size = GetFileVersionInfoSizeW(pcw(&w), None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(pcw(&w), None, size, buf.as_mut_ptr() as _).ok()?;
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let q = wide("\\VarFileInfo\\Translation");
        let mut lang = (0x0409u16, 0x04B0u16);
        if VerQueryValueW(buf.as_ptr() as _, pcw(&q), &mut ptr, &mut len).as_bool() && len >= 4 {
            let p = ptr as *const u16;
            lang = (*p, *p.add(1));
        }
        for (l, cp) in [lang, (0x0409, 0x04B0), (0x0416, 0x04B0), (0x0409, 0x04E4)] {
            let q = wide(&format!("\\StringFileInfo\\{:04x}{:04x}\\FileDescription", l, cp));
            if VerQueryValueW(buf.as_ptr() as _, pcw(&q), &mut ptr, &mut len).as_bool() && len > 1 {
                let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr as *const u16, len as usize - 1));
                let s = s.trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    }
}
