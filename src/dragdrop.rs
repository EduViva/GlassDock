// Arrastar e soltar (receber arquivos na dock, arrastar para fora),
// Lixeira, área de transferência de arquivos e seletores de arquivo.
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::*;
use windows::Win32::System::Ole::*;
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::*;

use crate::util::*;

pub fn files_from_dataobject(d: &IDataObject) -> Vec<String> {
    let mut out = vec![];
    unsafe {
        let fmt = FORMATETC { cfFormat: 15, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex: -1, tymed: TYMED_HGLOBAL.0 as u32 };
        if let Ok(mut med) = d.GetData(&fmt) {
            let hdrop = HDROP(med.u.hGlobal.0);
            let n = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
            for i in 0..n {
                let mut buf = [0u16; 1024];
                let len = DragQueryFileW(hdrop, i, Some(&mut buf));
                out.push(String::from_utf16_lossy(&buf[..len as usize]));
            }
            ReleaseStgMedium(&mut med);
        }
    }
    out
}

#[implement(IDropTarget)]
pub struct DockDrop {
    pub dock: usize,
}

fn has_files(d: Option<&IDataObject>) -> bool {
    let Some(d) = d else { return false };
    unsafe {
        let fmt = FORMATETC { cfFormat: 15, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex: -1, tymed: TYMED_HGLOBAL.0 as u32 };
        d.QueryGetData(&fmt) == S_OK
    }
}

impl IDropTarget_Impl for DockDrop_Impl {
    fn DragEnter(&self, d: windows_core::Ref<IDataObject>, _k: MODIFIERKEYS_FLAGS, pt: &POINTL, eff: *mut DROPEFFECT) -> Result<()> {
        let ok = has_files(d.as_ref());
        unsafe {
            *eff = if ok { crate::app::drop_hover(self.dock, pt.x, pt.y) } else { DROPEFFECT_NONE };
        }
        Ok(())
    }
    fn DragOver(&self, _k: MODIFIERKEYS_FLAGS, pt: &POINTL, eff: *mut DROPEFFECT) -> Result<()> {
        unsafe {
            *eff = crate::app::drop_hover(self.dock, pt.x, pt.y);
        }
        Ok(())
    }
    fn DragLeave(&self) -> Result<()> {
        crate::app::drop_leave(self.dock);
        Ok(())
    }
    fn Drop(&self, d: windows_core::Ref<IDataObject>, _k: MODIFIERKEYS_FLAGS, pt: &POINTL, eff: *mut DROPEFFECT) -> Result<()> {
        let files = d.as_ref().map(files_from_dataobject).unwrap_or_default();
        let e = crate::app::drop_files(self.dock, pt.x, pt.y, files);
        unsafe {
            *eff = e;
        }
        Ok(())
    }
}

pub fn register(hwnd: HWND, dock: usize) {
    unsafe {
        let t: IDropTarget = DockDrop { dock }.into();
        let _ = RegisterDragDrop(hwnd, &t);
    }
}

pub fn unregister(hwnd: HWND) {
    unsafe {
        let _ = RevokeDragDrop(hwnd);
    }
}

fn double_null(paths: &[String]) -> Vec<u16> {
    let mut v = vec![];
    for p in paths {
        v.extend(p.encode_utf16());
        v.push(0);
    }
    v.push(0);
    v
}

/// Manda para a Lixeira (com desfazer).
pub fn recycle(paths: &[String]) {
    unsafe {
        let from = double_null(paths);
        let mut op = SHFILEOPSTRUCTW {
            wFunc: FO_DELETE as u32,
            pFrom: PCWSTR(from.as_ptr()),
            fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT).0 as u16,
            ..Default::default()
        };
        let _ = SHFileOperationW(&mut op);
    }
}

/// Copia (ou move) arquivos para uma pasta, com a interface do Windows.
pub fn copy_into(paths: &[String], dir: &str, mv: bool) {
    unsafe {
        let from = double_null(paths);
        let to = double_null(&[dir.to_string()]);
        let mut op = SHFILEOPSTRUCTW {
            wFunc: if mv { FO_MOVE as u32 } else { FO_COPY as u32 },
            pFrom: PCWSTR(from.as_ptr()),
            pTo: PCWSTR(to.as_ptr()),
            fFlags: FOF_ALLOWUNDO.0 as u16,
            ..Default::default()
        };
        let _ = SHFileOperationW(&mut op);
    }
}

pub fn empty_recycle_bin(owner: HWND) {
    unsafe {
        let _ = SHEmptyRecycleBinW(Some(owner), PCWSTR::null(), 0);
    }
}

pub fn recycle_bin_full() -> bool {
    unsafe {
        let mut q = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
        SHQueryRecycleBinW(PCWSTR::null(), &mut q).is_ok() && q.i64NumItems > 0
    }
}

pub fn copy_files_to_clipboard(paths: &[String]) {
    unsafe {
        let list = double_null(paths);
        let header = std::mem::size_of::<DROPFILES>();
        let bytes = header + list.len() * 2;
        let Ok(h) = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes) else { return };
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            return;
        }
        let df = DROPFILES { pFiles: header as u32, pt: POINT::default(), fNC: false.into(), fWide: true.into() };
        std::ptr::copy_nonoverlapping(&df as *const _ as *const u8, p, header);
        std::ptr::copy_nonoverlapping(list.as_ptr() as *const u8, p.add(header), list.len() * 2);
        let _ = GlobalUnlock(h);
        if OpenClipboard(None).is_ok() {
            let _ = EmptyClipboard();
            let _ = SetClipboardData(15, Some(HANDLE(h.0)));
            let _ = CloseClipboard();
        }
    }
}

/// Começa a arrastar arquivos para fora (modal; chamar fora do estado do app).
pub fn drag_files_out(hwnd: HWND, paths: &[String]) {
    unsafe {
        let mut pidls: Vec<*const ITEMIDLIST> = vec![];
        for p in paths {
            let w = wide(p);
            let pidl = ILCreateFromPathW(pcw(&w));
            if !pidl.is_null() {
                pidls.push(pidl as *const _);
            }
        }
        if pidls.is_empty() {
            return;
        }
        if let Ok(arr) = SHCreateShellItemArrayFromIDLists(&pidls) {
            if let Ok(dobj) = arr.BindToHandler::<_, IDataObject>(None, &BHID_DataObject) {
                let _ = SHDoDragDrop(Some(hwnd), &dobj, None, DROPEFFECT_COPY | DROPEFFECT_MOVE | DROPEFFECT_LINK);
            }
        }
        for p in pidls {
            ILFree(Some(p));
        }
    }
}

pub enum PickKind {
    Folder,
    Exe,
    Any,
}

/// Seletor de arquivo/pasta do Windows (modal).
pub fn pick(owner: HWND, kind: PickKind, multi: bool) -> Vec<String> {
    let mut out = vec![];
    unsafe {
        let Ok(dlg) = CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) else { return out };
        let mut opts = dlg.GetOptions().unwrap_or_default();
        match kind {
            PickKind::Folder => opts |= FOS_PICKFOLDERS,
            PickKind::Exe => {
                let n1 = wide("Programas e atalhos");
                let f1 = wide("*.exe;*.lnk;*.bat;*.cmd;*.url");
                let n2 = wide("Todos os arquivos");
                let f2 = wide("*.*");
                let specs = [
                    windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC { pszName: pcw(&n1), pszSpec: pcw(&f1) },
                    windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC { pszName: pcw(&n2), pszSpec: pcw(&f2) },
                ];
                let _ = dlg.SetFileTypes(&specs);
                opts |= FOS_NODEREFERENCELINKS;
            }
            PickKind::Any => {}
        }
        if multi {
            opts |= FOS_ALLOWMULTISELECT;
        }
        let _ = dlg.SetOptions(opts | FOS_FORCEFILESYSTEM);
        if dlg.Show(Some(owner)).is_err() {
            return out;
        }
        if let Ok(items) = dlg.GetResults() {
            let n = items.GetCount().unwrap_or(0);
            for i in 0..n {
                if let Ok(it) = items.GetItemAt(i) {
                    if let Ok(p) = it.GetDisplayName(SIGDN_FILESYSPATH) {
                        out.push(from_pwstr(p));
                        CoTaskMemFree(Some(p.0 as _));
                    }
                }
            }
        }
    }
    out
}
