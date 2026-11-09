// HTTP GET simples (WinHTTP) para o clima e calendários ICS.
// Sempre chamado de uma thread de fundo.
use windows::core::*;
use windows::Win32::Networking::WinHttp::*;

use crate::util::*;

pub fn get(url: &str) -> Option<Vec<u8>> {
    unsafe {
        let wurl = wide(url);
        let mut host = vec![0u16; 256];
        let mut path = vec![0u16; 4096];
        let mut uc = URL_COMPONENTS {
            dwStructSize: std::mem::size_of::<URL_COMPONENTS>() as u32,
            lpszHostName: PWSTR(host.as_mut_ptr()),
            dwHostNameLength: host.len() as u32,
            lpszUrlPath: PWSTR(path.as_mut_ptr()),
            dwUrlPathLength: path.len() as u32,
            ..Default::default()
        };
        let mut extra = vec![0u16; 4096];
        uc.lpszExtraInfo = PWSTR(extra.as_mut_ptr());
        uc.dwExtraInfoLength = extra.len() as u32;
        WinHttpCrackUrl(&wurl[..wurl.len() - 1], 0, &mut uc).ok()?;
        let host_s = String::from_utf16_lossy(&host[..uc.dwHostNameLength as usize]);
        let path_s = String::from_utf16_lossy(&path[..uc.dwUrlPathLength as usize])
            + &String::from_utf16_lossy(&extra[..uc.dwExtraInfoLength as usize]);
        let secure = uc.nScheme == WINHTTP_INTERNET_SCHEME_HTTPS;
        let agent = wide("GlassDock/1.0");
        let ses = WinHttpOpen(pcw(&agent), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0);
        if ses.is_null() {
            return None;
        }
        let _ = WinHttpSetTimeouts(ses, 8000, 8000, 10000, 15000);
        let hw = wide(&host_s);
        let con = WinHttpConnect(ses, pcw(&hw), uc.nPort, 0);
        let mut out = None;
        if !con.is_null() {
            let pw = wide(&path_s);
            let get = wide("GET");
            let req = WinHttpOpenRequest(
                con,
                pcw(&get),
                pcw(&pw),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                if secure { WINHTTP_FLAG_SECURE } else { WINHTTP_OPEN_REQUEST_FLAGS(0) },
            );
            if !req.is_null() {
                if WinHttpSendRequest(req, None, None, 0, 0, 0).is_ok() && WinHttpReceiveResponse(req, std::ptr::null_mut()).is_ok() {
                    let mut status: u32 = 0;
                    let mut sz = 4u32;
                    let _ = WinHttpQueryHeaders(
                        req,
                        WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                        PCWSTR::null(),
                        Some(&mut status as *mut _ as _),
                        &mut sz,
                        std::ptr::null_mut(),
                    );
                    let mut body = vec![];
                    loop {
                        let mut avail = 0u32;
                        if WinHttpQueryDataAvailable(req, &mut avail).is_err() || avail == 0 {
                            break;
                        }
                        let mut chunk = vec![0u8; avail as usize];
                        let mut read = 0u32;
                        if WinHttpReadData(req, chunk.as_mut_ptr() as _, avail, &mut read).is_err() {
                            break;
                        }
                        body.extend_from_slice(&chunk[..read as usize]);
                        if body.len() > 16 * 1024 * 1024 {
                            break;
                        }
                    }
                    if (200..300).contains(&status) {
                        out = Some(body);
                    }
                }
                let _ = WinHttpCloseHandle(req);
            }
            let _ = WinHttpCloseHandle(con);
        }
        let _ = WinHttpCloseHandle(ses);
        out
    }
}

pub fn get_json(url: &str) -> Option<serde_json::Value> {
    let b = get(url)?;
    serde_json::from_slice(&b).ok()
}

pub fn url_encode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => o.push(b as char),
            _ => o.push_str(&format!("%{:02X}", b)),
        }
    }
    o
}
