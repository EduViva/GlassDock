// Motor gráfico: Direct2D (software) + DirectWrite + WIC, desenhando em DIBs
// apresentados com UpdateLayeredWindow. Nada roda continuamente: só desenha
// quando algo muda.
use std::cell::RefCell;
use std::collections::HashMap;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::util::*;
pub use windows_numerics::{Matrix3x2, Vector2};

#[inline]
pub fn pt(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn r(&self) -> f32 {
        self.x + self.w
    }
    pub fn b(&self) -> f32 {
        self.y + self.h
    }
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.r() && py >= self.y && py < self.b()
    }
    pub fn inset(&self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - 2.0 * d).max(0.0), (self.h - 2.0 * d).max(0.0))
    }
    pub fn inset_xy(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, (self.w - 2.0 * dx).max(0.0), (self.h - 2.0 * dy).max(0.0))
    }
    pub fn offset(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
    pub fn d2d(&self) -> D2D_RECT_F {
        D2D_RECT_F { left: self.x, top: self.y, right: self.r(), bottom: self.b() }
    }
    pub fn split_left(&self, w: f32) -> (Rect, Rect) {
        (Rect::new(self.x, self.y, w, self.h), Rect::new(self.x + w, self.y, (self.w - w).max(0.0), self.h))
    }
    pub fn split_top(&self, h: f32) -> (Rect, Rect) {
        (Rect::new(self.x, self.y, self.w, h), Rect::new(self.x, self.y + h, self.w, (self.h - h).max(0.0)))
    }
    pub fn centered(&self, w: f32, h: f32) -> Rect {
        Rect::new(self.cx() - w / 2.0, self.cy() - h / 2.0, w, h)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub f32, pub f32, pub f32, pub f32);

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
        Color(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
    }
    pub const fn hex(v: u32) -> Color {
        Color(((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0, 1.0)
    }
    pub fn a(self, a: f32) -> Color {
        Color(self.0, self.1, self.2, self.3 * a)
    }
    pub fn mix(self, o: Color, t: f32) -> Color {
        Color(lerp(self.0, o.0, t), lerp(self.1, o.1, t), lerp(self.2, o.2, t), lerp(self.3, o.3, t))
    }
    pub fn d2d(self) -> D2D1_COLOR_F {
        D2D1_COLOR_F { r: self.0, g: self.1, b: self.2, a: self.3 }
    }
    pub fn to_hex(self) -> String {
        format!(
            "#{:02X}{:02X}{:02X}",
            (self.0 * 255.0).round() as u8,
            (self.1 * 255.0).round() as u8,
            (self.2 * 255.0).round() as u8
        )
    }
    pub fn luminance(self) -> f32 {
        0.2126 * self.0 + 0.7152 * self.1 + 0.0722 * self.2
    }
}

pub const WHITE: Color = Color(1.0, 1.0, 1.0, 1.0);
pub const BLACK: Color = Color(0.0, 0.0, 0.0, 1.0);
pub const TRANSPARENT: Color = Color(0.0, 0.0, 0.0, 0.0);
pub const ACCENT: Color = Color::hex(0x0A84FF);
pub const RED: Color = Color::hex(0xFF453A);
pub const GREEN: Color = Color::hex(0x30D158);
pub const ORANGE: Color = Color::hex(0xFF9F0A);
pub const YELLOW: Color = Color::hex(0xFFD60A);
pub const PURPLE: Color = Color::hex(0xBF5AF2);
pub const PINK: Color = Color::hex(0xFF375F);
pub const TEAL: Color = Color::hex(0x40C8E0);
pub const GRAY: Color = Color::hex(0x8E8E93);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

pub const FONT_UI: &str = "Segoe UI Variable Text";
pub const FONT_DISPLAY: &str = "Segoe UI Variable Display";
pub const FONT_ICONS: &str = "Segoe Fluent Icons";
pub const FONT_ICONS_FALLBACK: &str = "Segoe MDL2 Assets";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FmtKey {
    size_x10: u32,
    weight: u32,
    family: u8,
    align: Align,
    valign: VAlign,
    wrap: bool,
}

pub struct Gfx {
    pub d2d: ID2D1Factory,
    pub dw: IDWriteFactory,
    pub wic: IWICImagingFactory,
    pub rt: ID2D1DCRenderTarget,
    pub round_stroke: ID2D1StrokeStyle,
    formats: RefCell<HashMap<FmtKey, IDWriteTextFormat>>,
    brush: RefCell<Option<ID2D1SolidColorBrush>>,
    icon_family: &'static str,
    ui_family: &'static str,
    display_family: &'static str,
}

thread_local! {
    static GFX: RefCell<Option<&'static Gfx>> = const { RefCell::new(None) };
}

pub fn gfx() -> &'static Gfx {
    GFX.with(|g| {
        if let Some(x) = *g.borrow() {
            return x;
        }
        let x: &'static Gfx = Box::leak(Box::new(Gfx::new().expect("Direct2D indisponível")));
        *g.borrow_mut() = Some(x);
        x
    })
}

fn font_exists(dw: &IDWriteFactory, name: &str) -> bool {
    unsafe {
        let mut coll = None;
        if dw.GetSystemFontCollection(&mut coll, false).is_err() {
            return false;
        }
        let Some(coll) = coll else { return false };
        let w = wide(name);
        let mut idx = 0u32;
        let mut exists = BOOL(0);
        coll.FindFamilyName(pcw(&w), &mut idx, &mut exists).is_ok() && exists.as_bool()
    }
}

impl Gfx {
    fn new() -> Result<Gfx> {
        unsafe {
            let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dw: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let rt = d2d.CreateDCRenderTarget(&props)?;
            rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let sp = D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 10.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            };
            let round_stroke = d2d.CreateStrokeStyle(&sp, None)?;
            let icon_family = if font_exists(&dw, FONT_ICONS) { FONT_ICONS } else { FONT_ICONS_FALLBACK };
            let ui_family = if font_exists(&dw, FONT_UI) { FONT_UI } else { "Segoe UI" };
            let display_family = if font_exists(&dw, FONT_DISPLAY) { FONT_DISPLAY } else { "Segoe UI" };
            Ok(Gfx {
                d2d,
                dw,
                wic,
                rt,
                round_stroke,
                formats: RefCell::new(HashMap::new()),
                brush: RefCell::new(None),
                icon_family,
                ui_family,
                display_family,
            })
        }
    }

    fn format(&self, size: f32, weight: u32, family: u8, align: Align, valign: VAlign, wrap: bool) -> IDWriteTextFormat {
        let key = FmtKey { size_x10: (size * 10.0).round() as u32, weight, family, align, valign, wrap };
        if let Some(f) = self.formats.borrow().get(&key) {
            return f.clone();
        }
        let fam = match family {
            1 => self.icon_family,
            2 => self.display_family,
            3 => "Segoe UI Emoji",
            _ => self.ui_family,
        };
        unsafe {
            let fw = wide(fam);
            let loc = wide("pt-br");
            let f = self
                .dw
                .CreateTextFormat(
                    pcw(&fw),
                    None,
                    DWRITE_FONT_WEIGHT(weight as i32),
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size.max(1.0),
                    pcw(&loc),
                )
                .expect("CreateTextFormat");
            let _ = f.SetTextAlignment(match align {
                Align::Left => DWRITE_TEXT_ALIGNMENT_LEADING,
                Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                Align::Right => DWRITE_TEXT_ALIGNMENT_TRAILING,
            });
            let _ = f.SetParagraphAlignment(match valign {
                VAlign::Top => DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
                VAlign::Center => DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                VAlign::Bottom => DWRITE_PARAGRAPH_ALIGNMENT_FAR,
            });
            let _ = f.SetWordWrapping(if wrap { DWRITE_WORD_WRAPPING_WRAP } else { DWRITE_WORD_WRAPPING_NO_WRAP });
            if !wrap {
                let sign = self.dw.CreateEllipsisTrimmingSign(&f).ok();
                let trim = DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, delimiter: 0, delimiterCount: 0 };
                let _ = f.SetTrimming(&trim, sign.as_ref());
            }
            self.formats.borrow_mut().insert(key, f.clone());
            f
        }
    }

    pub fn brush(&self, c: Color) -> ID2D1SolidColorBrush {
        let mut b = self.brush.borrow_mut();
        unsafe {
            if let Some(br) = b.as_ref() {
                br.SetColor(&c.d2d());
                return br.clone();
            }
            let br = self.rt.CreateSolidColorBrush(&c.d2d(), None).expect("brush");
            *b = Some(br.clone());
            br
        }
    }

    /// Mede um texto de uma linha.
    pub fn measure(&self, text: &str, size: f32, weight: u32) -> (f32, f32) {
        self.measure_fam(text, size, weight, 0)
    }

    pub fn measure_fam(&self, text: &str, size: f32, weight: u32, family: u8) -> (f32, f32) {
        let f = self.format(size, weight, family, Align::Left, VAlign::Top, false);
        let w = wide_nonul(text);
        unsafe {
            if let Ok(layout) = self.dw.CreateTextLayout(&w, &f, 10000.0, 10000.0) {
                let mut m = DWRITE_TEXT_METRICS::default();
                if layout.GetMetrics(&mut m).is_ok() {
                    return (m.widthIncludingTrailingWhitespace, m.height);
                }
            }
        }
        (0.0, 0.0)
    }

    pub fn measure_wrapped(&self, text: &str, size: f32, weight: u32, max_w: f32) -> (f32, f32) {
        let f = self.format(size, weight, 0, Align::Left, VAlign::Top, true);
        let w = wide_nonul(text);
        unsafe {
            if let Ok(layout) = self.dw.CreateTextLayout(&w, &f, max_w.max(1.0), 10000.0) {
                let mut m = DWRITE_TEXT_METRICS::default();
                if layout.GetMetrics(&mut m).is_ok() {
                    return (m.width, m.height);
                }
            }
        }
        (0.0, 0.0)
    }

    pub fn layout(&self, text: &str, size: f32, weight: u32, max_w: f32, wrap: bool) -> Option<IDWriteTextLayout> {
        let f = self.format(size, weight, 0, Align::Left, VAlign::Top, wrap);
        let w = wide_nonul(text);
        unsafe { self.dw.CreateTextLayout(&w, &f, max_w.max(1.0), 100000.0).ok() }
    }

    /// Converte um HBITMAP 32bpp em bitmap D2D.
    pub fn bitmap_from_hbitmap(&self, hbm: HBITMAP) -> Option<ID2D1Bitmap> {
        unsafe {
            let wb = self.wic.CreateBitmapFromHBITMAP(hbm, HPALETTE::default(), WICBitmapUsePremultipliedAlpha).ok()?;
            self.bitmap_from_wic(&wb.cast().ok()?)
        }
    }

    pub fn bitmap_from_hicon(&self, hicon: HICON) -> Option<ID2D1Bitmap> {
        unsafe {
            let wb = self.wic.CreateBitmapFromHICON(hicon).ok()?;
            self.bitmap_from_wic(&wb.cast().ok()?)
        }
    }

    pub fn bitmap_from_wic(&self, src: &IWICBitmapSource) -> Option<ID2D1Bitmap> {
        unsafe {
            let conv = self.wic.CreateFormatConverter().ok()?;
            conv.Initialize(src, &GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeMedianCut)
                .ok()?;
            self.rt.CreateBitmapFromWicBitmap(&conv, None).ok()
        }
    }

    /// Carrega uma imagem de arquivo (png/jpg) reduzida para no máximo `max` px.
    pub fn bitmap_from_file(&self, path: &str, max: u32) -> Option<ID2D1Bitmap> {
        unsafe {
            let w = wide(path);
            let dec = self
                .wic
                .CreateDecoderFromFilename(pcw(&w), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
                .ok()?;
            let frame = dec.GetFrame(0).ok()?;
            self.bitmap_scaled(&frame.cast().ok()?, max)
        }
    }

    pub fn bitmap_from_bytes(&self, bytes: &[u8], max: u32) -> Option<ID2D1Bitmap> {
        unsafe {
            let stream = self.wic.CreateStream().ok()?;
            stream.InitializeFromMemory(bytes).ok()?;
            let dec = self.wic.CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand).ok()?;
            let frame = dec.GetFrame(0).ok()?;
            self.bitmap_scaled(&frame.cast().ok()?, max)
        }
    }

    fn bitmap_scaled(&self, src: &IWICBitmapSource, max: u32) -> Option<ID2D1Bitmap> {
        unsafe {
            let (mut w, mut h) = (0u32, 0u32);
            src.GetSize(&mut w, &mut h).ok()?;
            if w == 0 || h == 0 {
                return None;
            }
            if w > max || h > max {
                let s = max as f32 / w.max(h) as f32;
                let nw = ((w as f32 * s) as u32).max(1);
                let nh = ((h as f32 * s) as u32).max(1);
                let sc = self.wic.CreateBitmapScaler().ok()?;
                sc.Initialize(src, nw, nh, WICBitmapInterpolationModeFant).ok()?;
                return self.bitmap_from_wic(&sc.cast().ok()?);
            }
            self.bitmap_from_wic(src)
        }
    }

    pub fn path(&self) -> Option<(ID2D1PathGeometry, ID2D1GeometrySink)> {
        unsafe {
            let g = self.d2d.CreatePathGeometry().ok()?;
            let s = g.Open().ok()?;
            Some((g, s))
        }
    }
}

// ---------------------------------------------------------------------------
// Superfície: DIB 32bpp + apresentação em janela "layered"
// ---------------------------------------------------------------------------

pub struct Surface {
    hdc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    pub w: i32,
    pub h: i32,
    scratch: Option<Box<Surface>>,
}

impl Surface {
    pub fn new() -> Surface {
        Surface { hdc: HDC::default(), bmp: HBITMAP::default(), old: HGDIOBJ::default(), bits: std::ptr::null_mut(), w: 0, h: 0, scratch: None }
    }

    fn ensure(&mut self, w: i32, h: i32) {
        let w = w.max(1);
        let h = h.max(1);
        if self.w == w && self.h == h && !self.hdc.is_invalid() {
            return;
        }
        unsafe {
            self.release();
            let screen = GetDC(None);
            self.hdc = CreateCompatibleDC(Some(screen));
            ReleaseDC(None, screen);
            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            self.bmp = CreateDIBSection(Some(self.hdc), &bi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap_or_default();
            self.bits = bits as *mut u8;
            self.old = SelectObject(self.hdc, self.bmp.into());
            self.w = w;
            self.h = h;
        }
    }

    fn release(&mut self) {
        unsafe {
            if !self.hdc.is_invalid() {
                SelectObject(self.hdc, self.old);
                let _ = DeleteObject(self.bmp.into());
                let _ = DeleteDC(self.hdc);
            }
            self.hdc = HDC::default();
        }
    }

    /// Desenha e apresenta na janela `hwnd` na posição (x, y) da tela.
    pub fn paint<F: FnOnce(&Canvas)>(&mut self, hwnd: HWND, x: i32, y: i32, w: i32, h: i32, f: F) {
        self.paint_dirty(hwnd, x, y, w, h, None, f)
    }

    /// Como `paint`, mas se `dirty` vier, só aquele retângulo é refeito e
    /// enviado ao Windows (o resto da imagem continua a mesma).
    pub fn paint_dirty<F: FnOnce(&Canvas)>(&mut self, hwnd: HWND, x: i32, y: i32, w: i32, h: i32, dirty: Option<Rect>, f: F) {
        let dirty = if self.w == w.max(1) && self.h == h.max(1) && !self.hdc.is_invalid() { dirty } else { None };
        self.ensure(w, h);
        let g = gfx();
        // região suja alinhada a pixels e dentro da superfície
        let drc = dirty.map(|d| RECT {
            left: (d.x.floor() as i32).clamp(0, self.w),
            top: (d.y.floor() as i32).clamp(0, self.h),
            right: (d.r().ceil() as i32).clamp(0, self.w),
            bottom: (d.b().ceil() as i32).clamp(0, self.h),
        }).filter(|r| r.right > r.left && r.bottom > r.top);
        unsafe {
            match drc {
                None => {
                    let rc = RECT { left: 0, top: 0, right: self.w, bottom: self.h };
                    if g.rt.BindDC(self.hdc, &rc).is_err() {
                        return;
                    }
                    g.rt.BeginDraw();
                    g.rt.Clear(Some(&TRANSPARENT.d2d()));
                    let c = Canvas { rt: g.rt.cast().unwrap(), g, dc: g.rt.cast().ok(), shape: std::cell::Cell::new(None) };
                    f(&c);
                    let _ = g.rt.EndDraw(None, None);
                }
                Some(r) => {
                    // desenha só a região numa superfície pequena e copia por cima
                    let (dw, dh) = (r.right - r.left, r.bottom - r.top);
                    let mut sc = self.scratch.take().unwrap_or_else(|| Box::new(Surface::new()));
                    sc.ensure(dw, dh);
                    let rc = RECT { left: 0, top: 0, right: dw, bottom: dh };
                    if g.rt.BindDC(sc.hdc, &rc).is_ok() {
                        g.rt.BeginDraw();
                        g.rt.SetTransform(&Matrix3x2 { M11: 1.0, M12: 0.0, M21: 0.0, M22: 1.0, M31: -(r.left as f32), M32: -(r.top as f32) });
                        g.rt.Clear(Some(&TRANSPARENT.d2d()));
                        let c = Canvas { rt: g.rt.cast().unwrap(), g, dc: g.rt.cast().ok(), shape: std::cell::Cell::new(None) };
                        f(&c);
                        g.rt.SetTransform(&Matrix3x2 { M11: 1.0, M12: 0.0, M21: 0.0, M22: 1.0, M31: 0.0, M32: 0.0 });
                        let _ = g.rt.EndDraw(None, None);
                        let _ = BitBlt(self.hdc, r.left, r.top, dw, dh, Some(sc.hdc), 0, 0, SRCCOPY);
                    }
                    self.scratch = Some(sc);
                }
            }
            let pt_dst = POINT { x, y };
            let size = SIZE { cx: self.w, cy: self.h };
            let pt_src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION { BlendOp: 0, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: 1 };
            let info = UPDATELAYEREDWINDOWINFO {
                cbSize: std::mem::size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
                hdcDst: HDC::default(),
                pptDst: &pt_dst,
                psize: &size,
                hdcSrc: self.hdc,
                pptSrc: &pt_src,
                crKey: COLORREF(0),
                pblend: &blend,
                dwFlags: ULW_ALPHA,
                prcDirty: drc.as_ref().map(|r| r as *const RECT).unwrap_or(std::ptr::null()),
            };
            let _ = UpdateLayeredWindowIndirect(hwnd, &info);
        }
    }

    /// Salva o que foi desenhado (modo de teste).
    pub fn save_png(&self, path: &str) {
        if self.bits.is_null() {
            return;
        }
        unsafe {
            let _ = GdiFlush();
            let g = gfx();
            let stride = (self.w * 4) as u32;
            let data = std::slice::from_raw_parts(self.bits, (self.w * self.h * 4) as usize);
            let Ok(bmp) = g.wic.CreateBitmapFromMemory(self.w as u32, self.h as u32, &GUID_WICPixelFormat32bppPBGRA, stride, data) else { return };
            let Ok(stream) = g.wic.CreateStream() else { return };
            let w = wide(path);
            if stream.InitializeFromFilename(pcw(&w), GENERIC_WRITE.0).is_err() {
                return;
            }
            let Ok(enc) = g.wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null()) else { return };
            let _ = enc.Initialize(&stream, WICBitmapEncoderNoCache);
            let mut frame = None;
            let _ = enc.CreateNewFrame(&mut frame, std::ptr::null_mut());
            if let Some(f) = frame {
                let _ = f.Initialize(None);
                let _ = f.WriteSource(&bmp, std::ptr::null());
                let _ = f.Commit();
            }
            let _ = enc.Commit();
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.release();
    }
}

// ---------------------------------------------------------------------------
// Canvas: primitivas de desenho
// ---------------------------------------------------------------------------

pub struct Canvas {
    pub rt: ID2D1RenderTarget,
    pub g: &'static Gfx,
    /// interface 1.1, para redimensionar imagens com interpolação cúbica
    pub dc: Option<ID2D1DeviceContext>,
    /// "molde": quando um retângulo arredondado coincide com este, usa
    /// estes raios por canto [sup. esq., sup. dir., inf. dir., inf. esq.]
    pub shape: std::cell::Cell<Option<(Rect, [f32; 4])>>,
}

pub const W_NORMAL: u32 = 400;
pub const W_MEDIUM: u32 = 500;
pub const W_SEMIBOLD: u32 = 600;
pub const W_BOLD: u32 = 700;

impl Canvas {
    pub fn fill_rect(&self, r: Rect, c: Color) {
        unsafe { self.rt.FillRectangle(&r.d2d(), &self.g.brush(c)) }
    }

    /// Raios por canto se `r` coincide com o molde atual.
    fn shape_for(&self, r: Rect, rad: f32) -> Option<[f32; 4]> {
        let (o, radii) = self.shape.get()?;
        if (r.cx() - o.cx()).abs() > 1.5 || (r.cy() - o.cy()).abs() > 1.5 || (r.w - o.w).abs() > 3.5 || (r.h - o.h).abs() > 3.5 {
            return None;
        }
        let shrink = ((o.w - r.w) / 2.0).max(0.0);
        let lim = (r.w.min(r.h) / 2.0).max(0.0);
        Some(radii.map(|k| (k - shrink).max(rad).min(lim)))
    }

    /// Geometria de retângulo com um raio por canto (cantos em Bézier).
    pub fn rrect4_geo(&self, r: Rect, k: [f32; 4]) -> Option<ID2D1PathGeometry> {
        let (geo, sink) = self.g.path()?;
        let [tl, tr, br, bl] = k;
        // constante do círculo por cúbicas: 4/3·(√2−1)
        let q = 0.552_284_8f32;
        unsafe {
            let bez = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| D2D1_BEZIER_SEGMENT { point1: pt(p1.0, p1.1), point2: pt(p2.0, p2.1), point3: pt(p3.0, p3.1) };
            sink.BeginFigure(pt(r.x + tl, r.y), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(pt(r.r() - tr, r.y));
            if tr > 0.0 {
                sink.AddBezier(&bez((r.r() - tr + tr * q, r.y), (r.r(), r.y + tr - tr * q), (r.r(), r.y + tr)));
            }
            sink.AddLine(pt(r.r(), r.b() - br));
            if br > 0.0 {
                sink.AddBezier(&bez((r.r(), r.b() - br + br * q), (r.r() - br + br * q, r.b()), (r.r() - br, r.b())));
            }
            sink.AddLine(pt(r.x + bl, r.b()));
            if bl > 0.0 {
                sink.AddBezier(&bez((r.x + bl - bl * q, r.b()), (r.x, r.b() - bl + bl * q), (r.x, r.b() - bl)));
            }
            sink.AddLine(pt(r.x, r.y + tl));
            if tl > 0.0 {
                sink.AddBezier(&bez((r.x, r.y + tl - tl * q), (r.x + tl - tl * q, r.y), (r.x + tl, r.y)));
            }
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            let _ = sink.Close();
        }
        Some(geo)
    }

    fn fill_shape_with(&self, r: Rect, k: [f32; 4], b: &ID2D1Brush) {
        if let Some(geo) = self.rrect4_geo(r, k) {
            unsafe { self.rt.FillGeometry(&geo, b, None) }
        }
    }

    pub fn fill_rrect(&self, r: Rect, rad: f32, c: Color) {
        if let Some(k) = self.shape_for(r, rad) {
            return self.fill_shape_with(r, k, &self.g.brush(c).cast().unwrap());
        }
        let rr = D2D1_ROUNDED_RECT { rect: r.d2d(), radiusX: rad, radiusY: rad };
        unsafe { self.rt.FillRoundedRectangle(&rr, &self.g.brush(c)) }
    }

    /// Retângulo arredondado com "furos" (para as miniaturas aparecerem por baixo).
    pub fn fill_rrect_holes(&self, r: Rect, rad: f32, holes: &[Rect], c: Color) {
        if holes.is_empty() {
            return self.fill_rrect(r, rad, c);
        }
        unsafe {
            let rr = D2D1_ROUNDED_RECT { rect: r.d2d(), radiusX: rad, radiusY: rad };
            let Ok(base) = self.g.d2d.CreateRoundedRectangleGeometry(&rr) else { return };
            let mut cur: ID2D1Geometry = base.cast().unwrap();
            for h in holes {
                let Ok(hg) = self.g.d2d.CreateRectangleGeometry(&h.d2d()) else { continue };
                let Some((pg, sink)) = self.g.path() else { continue };
                if cur.CombineWithGeometry(&hg, D2D1_COMBINE_MODE_EXCLUDE, None, 0.25, &sink).is_ok() {
                    let _ = sink.Close();
                    cur = pg.cast().unwrap();
                }
            }
            self.rt.FillGeometry(&cur, &self.g.brush(c), None);
        }
    }

    pub fn stroke_rrect(&self, r: Rect, rad: f32, c: Color, w: f32) {
        if let Some(k) = self.shape_for(r, rad) {
            if let Some(geo) = self.rrect4_geo(r, k) {
                unsafe { self.rt.DrawGeometry(&geo, &self.g.brush(c), w, None) };
            }
            return;
        }
        let rr = D2D1_ROUNDED_RECT { rect: r.d2d(), radiusX: rad, radiusY: rad };
        unsafe { self.rt.DrawRoundedRectangle(&rr, &self.g.brush(c), w, None) }
    }

    fn linear(&self, x0: f32, y0: f32, x1: f32, y1: f32, stops: &[(f32, Color)]) -> Option<ID2D1LinearGradientBrush> {
        // GLASSDOCK_TEST_NOALPHAGRAD: só para inspeção no Wine, que desenha
        // degradês translúcidos como opacos.
        if stops.iter().any(|s| s.1 .3 < 0.99) && std::env::var("GLASSDOCK_TEST_NOALPHAGRAD").is_ok() {
            return None;
        }
        unsafe {
            let st: Vec<D2D1_GRADIENT_STOP> = stops.iter().map(|(p, c)| D2D1_GRADIENT_STOP { position: *p, color: c.d2d() }).collect();
            let coll = self.rt.CreateGradientStopCollection(&st, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP).ok()?;
            let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: pt(x0, y0),
                endPoint: pt(x1, y1),
            };
            self.rt.CreateLinearGradientBrush(&props, None, &coll).ok()
        }
    }

    pub fn fill_rrect_vgrad(&self, r: Rect, rad: f32, stops: &[(f32, Color)]) {
        if let Some(b) = self.linear(r.x, r.y, r.x, r.b(), stops) {
            if let Some(k) = self.shape_for(r, rad) {
                return self.fill_shape_with(r, k, &b.cast().unwrap());
            }
            let rr = D2D1_ROUNDED_RECT { rect: r.d2d(), radiusX: rad, radiusY: rad };
            unsafe { self.rt.FillRoundedRectangle(&rr, &b) }
        }
    }

    /// Degradê com ângulo em graus (0 = de cima para baixo).
    pub fn fill_rrect_grad(&self, r: Rect, rad: f32, angle: f32, stops: &[(f32, Color)]) {
        let a = angle.to_radians();
        let (dx, dy) = (a.sin() * r.w / 2.0, a.cos() * r.h / 2.0);
        if let Some(b) = self.linear(r.cx() - dx, r.cy() - dy, r.cx() + dx, r.cy() + dy, stops) {
            if let Some(k) = self.shape_for(r, rad) {
                return self.fill_shape_with(r, k, &b.cast().unwrap());
            }
            let rr = D2D1_ROUNDED_RECT { rect: r.d2d(), radiusX: rad, radiusY: rad };
            unsafe { self.rt.FillRoundedRectangle(&rr, &b) }
        }
    }

    pub fn stroke_rrect_vgrad(&self, r: Rect, rad: f32, w: f32, stops: &[(f32, Color)]) {
        if let Some(b) = self.linear(r.x, r.y, r.x, r.b(), stops) {
            let rr = D2D1_ROUNDED_RECT { rect: r.d2d(), radiusX: rad, radiusY: rad };
            unsafe { self.rt.DrawRoundedRectangle(&rr, &b, w, None) }
        }
    }

    pub fn fill_radial(&self, cx: f32, cy: f32, rx: f32, ry: f32, area: Rect, rad: f32, stops: &[(f32, Color)]) {
        unsafe {
            let st: Vec<D2D1_GRADIENT_STOP> = stops.iter().map(|(p, c)| D2D1_GRADIENT_STOP { position: *p, color: c.d2d() }).collect();
            let Ok(coll) = self.rt.CreateGradientStopCollection(&st, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP) else { return };
            let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                center: pt(cx, cy),
                gradientOriginOffset: pt(0.0, 0.0),
                radiusX: rx,
                radiusY: ry,
            };
            if let Ok(b) = self.rt.CreateRadialGradientBrush(&props, None, &coll) {
                let rr = D2D1_ROUNDED_RECT { rect: area.d2d(), radiusX: rad, radiusY: rad };
                self.rt.FillRoundedRectangle(&rr, &b);
            }
        }
    }

    /// Sombra suave aproximada por camadas.
    pub fn shadow(&self, r: Rect, rad: f32, blur: f32, dy: f32, c: Color) {
        let steps = 8;
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let grow = blur * (1.0 - t);
            let alpha = c.3 / steps as f32 * (0.35 + t * 0.9);
            self.fill_rrect(
                Rect::new(r.x - grow, r.y - grow + dy, r.w + 2.0 * grow, r.h + 2.0 * grow),
                rad + grow,
                c.a(alpha / c.3.max(0.001)),
            );
        }
    }

    pub fn fill_circle(&self, cx: f32, cy: f32, r: f32, c: Color) {
        let e = D2D1_ELLIPSE { point: pt(cx, cy), radiusX: r, radiusY: r };
        unsafe { self.rt.FillEllipse(&e, &self.g.brush(c)) }
    }

    pub fn fill_ellipse(&self, cx: f32, cy: f32, rx: f32, ry: f32, c: Color) {
        let e = D2D1_ELLIPSE { point: pt(cx, cy), radiusX: rx, radiusY: ry };
        unsafe { self.rt.FillEllipse(&e, &self.g.brush(c)) }
    }

    pub fn stroke_circle(&self, cx: f32, cy: f32, r: f32, c: Color, w: f32) {
        let e = D2D1_ELLIPSE { point: pt(cx, cy), radiusX: r, radiusY: r };
        unsafe { self.rt.DrawEllipse(&e, &self.g.brush(c), w, None) }
    }

    pub fn line(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color, w: f32) {
        unsafe {
            self.rt.DrawLine(
                pt(x0, y0),
                pt(x1, y1),
                &self.g.brush(c),
                w,
                &self.g.round_stroke,
            )
        }
    }

    /// Arco (anel de progresso). `start` e `sweep` em frações de volta; 0 = topo.
    pub fn arc(&self, cx: f32, cy: f32, r: f32, start: f32, sweep: f32, c: Color, w: f32) {
        let sweep = sweep.max(-0.9999).min(0.9999);
        if sweep.abs() < 0.001 {
            return;
        }
        let ap = |f: f32| {
            let a = (f * 360.0 - 90.0).to_radians();
            pt(cx + r * a.cos(), cy + r * a.sin())
        };
        let Some((geo, sink)) = self.g.path() else { return };
        unsafe {
            sink.BeginFigure(ap(start), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: ap(start + sweep),
                size: D2D_SIZE_F { width: r, height: r },
                rotationAngle: 0.0,
                sweepDirection: if sweep > 0.0 { D2D1_SWEEP_DIRECTION_CLOCKWISE } else { D2D1_SWEEP_DIRECTION_COUNTER_CLOCKWISE },
                arcSize: if sweep.abs() > 0.5 { D2D1_ARC_SIZE_LARGE } else { D2D1_ARC_SIZE_SMALL },
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            let _ = sink.Close();
            self.rt.DrawGeometry(&geo, &self.g.brush(c), w, &self.g.round_stroke);
        }
    }

    pub fn ring(&self, cx: f32, cy: f32, r: f32, frac: f32, track: Color, c: Color, w: f32) {
        self.stroke_circle(cx, cy, r, track, w);
        if frac >= 0.999 {
            self.stroke_circle(cx, cy, r, c, w);
        } else {
            self.arc(cx, cy, r, 0.0, frac, c, w);
        }
    }

    /// Fatia de pizza preenchida (frações de volta).
    pub fn pie(&self, cx: f32, cy: f32, r: f32, start: f32, sweep: f32, c: Color) {
        let sweep = sweep.max(0.0).min(0.9999);
        if sweep < 0.001 {
            return;
        }
        let ap = |f: f32| {
            let a = (f * 360.0 - 90.0).to_radians();
            pt(cx + r * a.cos(), cy + r * a.sin())
        };
        let Some((geo, sink)) = self.g.path() else { return };
        unsafe {
            sink.BeginFigure(pt(cx, cy), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(ap(start));
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: ap(start + sweep),
                size: D2D_SIZE_F { width: r, height: r },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: if sweep > 0.5 { D2D1_ARC_SIZE_LARGE } else { D2D1_ARC_SIZE_SMALL },
            });
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            let _ = sink.Close();
            self.rt.FillGeometry(&geo, &self.g.brush(c), None);
        }
    }

    /// Polígono preenchido.
    pub fn fill_poly(&self, pts: &[(f32, f32)], c: Color) {
        if pts.len() < 3 {
            return;
        }
        let Some((geo, sink)) = self.g.path() else { return };
        unsafe {
            sink.BeginFigure(pt(pts[0].0, pts[0].1), D2D1_FIGURE_BEGIN_FILLED);
            for p in &pts[1..] {
                sink.AddLine(pt(p.0, p.1));
            }
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            let _ = sink.Close();
            self.rt.FillGeometry(&geo, &self.g.brush(c), None);
        }
    }

    /// Linha poligonal (gráficos).
    pub fn polyline(&self, pts: &[(f32, f32)], c: Color, w: f32) {
        if pts.len() < 2 {
            return;
        }
        let Some((geo, sink)) = self.g.path() else { return };
        unsafe {
            sink.BeginFigure(pt(pts[0].0, pts[0].1), D2D1_FIGURE_BEGIN_HOLLOW);
            for p in &pts[1..] {
                sink.AddLine(pt(p.0, p.1));
            }
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            let _ = sink.Close();
            self.rt.DrawGeometry(&geo, &self.g.brush(c), w, &self.g.round_stroke);
        }
    }

    pub fn text_ex(&self, s: &str, r: Rect, size: f32, weight: u32, c: Color, align: Align, valign: VAlign, family: u8, wrap: bool) {
        if s.is_empty() || r.w <= 0.0 {
            return;
        }
        let f = self.g.format(size, weight, family, align, valign, wrap);
        let w = wide_nonul(s);
        let opts = if family == 3 { D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT } else { D2D1_DRAW_TEXT_OPTIONS_CLIP };
        unsafe { self.rt.DrawText(&w, &f, &r.d2d(), &self.g.brush(c), opts, DWRITE_MEASURING_MODE_NATURAL) }
    }

    pub fn text(&self, s: &str, r: Rect, size: f32, weight: u32, c: Color, align: Align) {
        self.text_ex(s, r, size, weight, c, align, VAlign::Center, 0, false)
    }

    pub fn text_wrap(&self, s: &str, r: Rect, size: f32, weight: u32, c: Color, align: Align) {
        self.text_ex(s, r, size, weight, c, align, VAlign::Top, 0, true)
    }

    pub fn display(&self, s: &str, r: Rect, size: f32, weight: u32, c: Color, align: Align) {
        self.text_ex(s, r, size, weight, c, align, VAlign::Center, 2, false)
    }

    /// Ícone de fonte (Segoe Fluent Icons / MDL2).
    pub fn glyph(&self, code: u32, r: Rect, size: f32, c: Color) {
        let s: String = char::from_u32(code).map(|c| c.to_string()).unwrap_or_default();
        self.text_ex(&s, r, size, W_NORMAL, c, Align::Center, VAlign::Center, 1, false)
    }

    pub fn emoji(&self, s: &str, r: Rect, size: f32) {
        self.text_ex(s, r, size, W_NORMAL, WHITE, Align::Center, VAlign::Center, 3, false)
    }

    pub fn layout_text(&self, layout: &IDWriteTextLayout, x: f32, y: f32, c: Color) {
        unsafe { self.rt.DrawTextLayout(pt(x, y), layout, &self.g.brush(c), D2D1_DRAW_TEXT_OPTIONS_NONE) }
    }

    pub fn bitmap(&self, b: &ID2D1Bitmap, r: Rect, opacity: f32) {
        unsafe {
            match &self.dc {
                Some(dc) => dc.DrawBitmap(b, Some(&r.d2d()), opacity, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, None, None),
                None => self.rt.DrawBitmap(b, Some(&r.d2d()), opacity, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None),
            }
        }
    }

    /// Desenha um bitmap preenchendo `r` (corta o excesso) com cantos arredondados.
    pub fn bitmap_cover_rounded(&self, b: &ID2D1Bitmap, r: Rect, rad: f32, opacity: f32) {
        unsafe {
            let sz = b.GetSize();
            if sz.width <= 0.0 || sz.height <= 0.0 {
                return;
            }
            let s = (r.w / sz.width).max(r.h / sz.height);
            let (dw, dh) = (sz.width * s, sz.height * s);
            let dst = Rect::new(r.cx() - dw / 2.0, r.cy() - dh / 2.0, dw, dh);
            self.push_rrect_clip(r, rad);
            self.bitmap(b, dst, opacity);
            self.pop_clip();
        }
    }

    pub fn push_rrect_clip(&self, r: Rect, rad: f32) {
        unsafe {
            let geo: ID2D1Geometry = match self.shape_for(r, rad).and_then(|k| self.rrect4_geo(r, k)) {
                Some(g) => g.cast().unwrap(),
                None => {
                    let rr = D2D1_ROUNDED_RECT { rect: r.d2d(), radiusX: rad, radiusY: rad };
                    let Ok(g) = self.g.d2d.CreateRoundedRectangleGeometry(&rr) else { return };
                    g.cast().unwrap()
                }
            };
            let params = D2D1_LAYER_PARAMETERS {
                contentBounds: r.d2d(),
                geometricMask: std::mem::ManuallyDrop::new(Some(geo.cast().unwrap())),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: Matrix3x2 { M11: 1.0, M12: 0.0, M21: 0.0, M22: 1.0, M31: 0.0, M32: 0.0 },
                opacity: 1.0,
                opacityBrush: std::mem::ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS_NONE,
            };
            self.rt.PushLayer(&params, None);
            let mut p = params;
            std::mem::ManuallyDrop::drop(&mut p.geometricMask);
        }
    }

    pub fn pop_clip(&self) {
        unsafe { self.rt.PopLayer() }
    }

    pub fn push_clip(&self, r: Rect) {
        unsafe { self.rt.PushAxisAlignedClip(&r.d2d(), D2D1_ANTIALIAS_MODE_ALIASED) }
    }

    pub fn pop_axis_clip(&self) {
        unsafe { self.rt.PopAxisAlignedClip() }
    }
}

/// Converte pontos lógicos (96 dpi) em pixels para um dado DPI.
pub fn scale(dpi: u32) -> f32 {
    dpi as f32 / 96.0
}

/// Registra uma classe de janela simples.
pub fn register_class(name: &str, proc: WNDPROC) {
    unsafe {
        let hinst = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
        let wname = wide(name);
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS,
            lpfnWndProc: proc,
            hInstance: hinst.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: pcw(&wname),
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            ..Default::default()
        };
        RegisterClassExW(&wc);
        std::mem::forget(wname);
    }
}
