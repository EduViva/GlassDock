// Utilitários gerais: strings wide, caminhos, tempo e datas.
use std::path::PathBuf;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::SystemInformation::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Shell::*;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn wide_nonul(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

pub fn from_wide(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

pub unsafe fn from_pwstr(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    p.to_string().unwrap_or_default()
}

pub fn pcw(v: &[u16]) -> PCWSTR {
    PCWSTR(v.as_ptr())
}

pub fn tick_ms() -> u64 {
    unsafe { GetTickCount64() }
}

/// Pasta de dados do app: %APPDATA%\GlassDock
pub fn data_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    let p = PathBuf::from(base).join("GlassDock");
    let _ = std::fs::create_dir_all(&p);
    p
}

pub fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".into()))
}

pub fn known_folder(id: &windows::core::GUID) -> Option<PathBuf> {
    unsafe {
        let p = SHGetKnownFolderPath(id, KNOWN_FOLDER_FLAG(0), None).ok()?;
        let s = from_pwstr(p);
        windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as _));
        Some(PathBuf::from(s))
    }
}

pub fn exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_default()
}

pub fn process_image_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        r.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

pub fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

// ---------------------------------------------------------------------------
// Datas (calendário civil, sem dependências externas)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DateTime {
    pub year: i32,
    pub month: u32, // 1..12
    pub day: u32,   // 1..31
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub ms: u32,
    pub dow: u32, // 0 = domingo
}

impl DateTime {
    pub fn from_systemtime(st: &SYSTEMTIME) -> Self {
        DateTime {
            year: st.wYear as i32,
            month: st.wMonth as u32,
            day: st.wDay as u32,
            hour: st.wHour as u32,
            minute: st.wMinute as u32,
            second: st.wSecond as u32,
            ms: st.wMilliseconds as u32,
            dow: st.wDayOfWeek as u32,
        }
    }
    pub fn to_systemtime(&self) -> SYSTEMTIME {
        SYSTEMTIME {
            wYear: self.year as u16,
            wMonth: self.month as u16,
            wDayOfWeek: self.dow as u16,
            wDay: self.day as u16,
            wHour: self.hour as u16,
            wMinute: self.minute as u16,
            wSecond: self.second as u16,
            wMilliseconds: self.ms as u16,
        }
    }
    pub fn days(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }
    /// segundos "locais" desde a época (sem fuso; só para diferenças entre datas locais)
    pub fn local_secs(&self) -> i64 {
        self.days() * 86400 + (self.hour * 3600 + self.minute * 60 + self.second) as i64
    }
    pub fn from_local_secs(s: i64) -> Self {
        let days = s.div_euclid(86400);
        let rem = s.rem_euclid(86400) as u32;
        let (y, m, d) = civil_from_days(days);
        DateTime {
            year: y,
            month: m,
            day: d,
            hour: rem / 3600,
            minute: (rem / 60) % 60,
            second: rem % 60,
            ms: 0,
            dow: weekday_from_days(days),
        }
    }
    pub fn date_only(&self) -> Self {
        let mut d = *self;
        d.hour = 0;
        d.minute = 0;
        d.second = 0;
        d.ms = 0;
        d
    }
}

pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

pub fn weekday_from_days(z: i64) -> u32 {
    (if z >= -4 { (z + 4) % 7 } else { (z + 5) % 7 + 6 }) as u32
}

pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

pub fn local_now() -> DateTime {
    unsafe { DateTime::from_systemtime(&GetLocalTime()) }
}

/// Segundos UNIX (UTC) agora.
pub fn unix_now() -> i64 {
    unsafe {
        let ft = GetSystemTimeAsFileTime();
        let v = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
        (v / 10_000_000) as i64 - 11_644_473_600
    }
}

/// Diferença atual (local - UTC) em segundos.
pub fn local_offset_secs() -> i64 {
    let utc = unsafe { GetSystemTime() };
    let loc = local_now();
    loc.local_secs() - DateTime::from_systemtime(&utc).local_secs()
}

pub fn unix_to_local(ts: i64) -> DateTime {
    DateTime::from_local_secs(ts + local_offset_secs())
}

pub fn local_to_unix(dt: &DateTime) -> i64 {
    dt.local_secs() - local_offset_secs()
}

pub const MESES: [&str; 12] = [
    "janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro",
    "outubro", "novembro", "dezembro",
];
pub const MESES_CURTO: [&str; 12] = [
    "JAN", "FEV", "MAR", "ABR", "MAI", "JUN", "JUL", "AGO", "SET", "OUT", "NOV", "DEZ",
];
pub const DIAS_CURTO: [&str; 7] = ["DOM", "SEG", "TER", "QUA", "QUI", "SEX", "SÁB"];
pub const DIAS: [&str; 7] = [
    "domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira",
    "sábado",
];

pub fn fmt_duration_short(secs: i64) -> String {
    let secs = secs.max(0);
    let h = secs / 3600;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}

pub fn fmt_hm(secs: i64) -> String {
    let secs = secs.max(0);
    let h = secs / 3600;
    let m = (secs / 60) % 60;
    if h > 0 {
        format!("{}h {:02}", h, m)
    } else {
        format!("{} min", m)
    }
}

pub fn fmt_bytes_rate(bps: f64) -> String {
    if bps >= 1e9 {
        format!("{:.1} GB/s", bps / 1e9)
    } else if bps >= 1e6 {
        format!("{:.1} MB/s", bps / 1e6)
    } else if bps >= 1e3 {
        format!("{:.0} KB/s", bps / 1e3)
    } else {
        format!("{:.0} B/s", bps)
    }
}

pub fn fmt_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out
}

pub fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1000 {
        format!("{:.1}k", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn clamp01(v: f32) -> f32 {
    v.max(0.0).min(1.0)
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let t = clamp01(t);
    1.0 - (1.0 - t).powi(3)
}

pub fn ease_in_out(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

pub fn shell_open(target: &str) {
    shell_exec(target, None, None);
}

pub fn shell_exec(target: &str, args: Option<&str>, dir: Option<&str>) -> bool {
    unsafe {
        let t = wide(target);
        let a = args.map(wide);
        let d = dir.map(wide);
        let open = wide("open");
        let r = ShellExecuteW(
            None,
            pcw(&open),
            pcw(&t),
            a.as_ref().map(|v| pcw(v)).unwrap_or(PCWSTR::null()),
            d.as_ref().map(|v| pcw(v)).unwrap_or(PCWSTR::null()),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
        r.0 as isize > 32
    }
}

/// ShellExecuteEx que só volta quando o Windows terminou de iniciar o app.
/// Feito para rodar numa thread própria (uma chamada assíncrona seria
/// cortada quando a thread termina, e o app demoraria ou nem abriria).
pub fn shell_exec_blocking(target: &str, args: Option<&str>, dir: Option<&str>) -> bool {
    unsafe {
        let t = wide(target);
        let a = args.map(wide);
        let d = dir.map(wide);
        let open = wide("open");
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI | SEE_MASK_NOCLOSEPROCESS,
            lpVerb: pcw(&open),
            lpFile: pcw(&t),
            lpParameters: a.as_ref().map(|v| pcw(v)).unwrap_or(PCWSTR::null()),
            lpDirectory: d.as_ref().map(|v| pcw(v)).unwrap_or(PCWSTR::null()),
            nShow: windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0,
            ..Default::default()
        };
        let ok = ShellExecuteExW(&mut info).is_ok();
        if !info.hProcess.is_invalid() {
            let _ = CloseHandle(info.hProcess);
        }
        ok
    }
}

/// Há quantos segundos o processo foi criado.
pub fn process_age_secs(pid: u32) -> Option<f64> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let r = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u);
        let _ = CloseHandle(h);
        r.ok()?;
        let created = ((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64;
        let now = GetSystemTimeAsFileTime();
        let now = ((now.dwHighDateTime as u64) << 32) | now.dwLowDateTime as u64;
        Some(now.saturating_sub(created) as f64 / 1e7)
    }
}

pub fn show_in_explorer(path: &str) {
    let arg = format!("/select,\"{}\"", path);
    shell_exec("explorer.exe", Some(&arg), None);
}

pub fn is_win11() -> bool {
    unsafe {
        let mut vi = OSVERSIONINFOW { dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32, ..Default::default() };
        type RtlGetVersion = unsafe extern "system" fn(*mut OSVERSIONINFOW) -> i32;
        let ntdll = windows::Win32::System::LibraryLoader::GetModuleHandleW(windows::core::w!("ntdll.dll"));
        if let Ok(h) = ntdll {
            if let Some(f) = windows::Win32::System::LibraryLoader::GetProcAddress(h, windows::core::s!("RtlGetVersion")) {
                let f: RtlGetVersion = std::mem::transmute(f);
                f(&mut vi);
                return vi.dwBuildNumber >= 22000;
            }
        }
        false
    }
}

/// Copia texto para a área de transferência.
pub fn set_clipboard_text(text: &str) {
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;
    unsafe {
        if OpenClipboard(None).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        let w = wide(text);
        let bytes = w.len() * 2;
        if let Ok(h) = GlobalAlloc(GMEM_MOVEABLE, bytes) {
            let p = GlobalLock(h) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(w.as_ptr(), p, w.len());
                let _ = GlobalUnlock(h);
                let _ = SetClipboardData(13 /* CF_UNICODETEXT */, Some(HANDLE(h.0)));
            }
        }
        let _ = CloseClipboard();
    }
}

pub fn get_clipboard_text() -> Option<String> {
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;
    unsafe {
        OpenClipboard(None).ok()?;
        let mut out = None;
        if let Ok(h) = GetClipboardData(13) {
            let p = GlobalLock(HGLOBAL(h.0)) as *const u16;
            if !p.is_null() {
                let mut n = 0;
                while *p.add(n) != 0 {
                    n += 1;
                }
                out = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, n)));
                let _ = GlobalUnlock(HGLOBAL(h.0));
            }
        }
        let _ = CloseClipboard();
        out
    }
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Option<T> {
    let s = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

pub fn write_json_atomic<T: serde::Serialize>(path: &std::path::Path, v: &T) {
    if let Ok(s) = serde_json::to_string_pretty(v) {
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

pub fn hwnd_ok(h: HWND) -> bool {
    !h.0.is_null()
}
