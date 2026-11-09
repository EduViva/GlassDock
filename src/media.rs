// Música tocando agora, de qualquer player que publica no Windows
// (Spotify, navegador, Música, etc.), pela sessão de mídia do sistema.
// Uma thread própria espera eventos; nada é consultado periodicamente.
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::*;
use windows::Storage::Streams::*;

use crate::app::{post, WM_APP_MEDIA};

#[derive(Clone, Default, Debug)]
pub struct MediaInfo {
    pub has_session: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub app_id: String,
    pub art: Option<std::sync::Arc<Vec<u8>>>,
    pub art_serial: u64,
    pub pos: f64,
    pub dur: f64,
    pub pos_at: u64,
}

impl MediaInfo {
    pub fn position_now(&self) -> f64 {
        if self.playing && self.dur > 0.0 {
            let dt = (crate::util::tick_ms().saturating_sub(self.pos_at)) as f64 / 1000.0;
            (self.pos + dt).min(self.dur)
        } else {
            self.pos
        }
    }
}

pub enum Cmd {
    Refresh,
    Props,
    Playback,
    PlayPause,
    Next,
    Prev,
    Seek(f64),
}

pub static SNAPSHOT: Mutex<Option<MediaInfo>> = Mutex::new(None);

pub fn start() -> Sender<Cmd> {
    let (tx, rx) = channel::<Cmd>();
    let tx2 = tx.clone();
    std::thread::spawn(move || {
        unsafe {
            let _ = windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);
        }
        run(rx, tx2);
    });
    tx
}

fn run(rx: Receiver<Cmd>, tx: Sender<Cmd>) {
    let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() else { return };
    let Ok(mgr) = op.join() else { return };
    let t = tx.clone();
    let _ = mgr.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
        let _ = t.send(Cmd::Refresh);
        Ok(())
    }));
    let mut info = MediaInfo::default();
    let mut session: Option<GlobalSystemMediaTransportControlsSession> = None;
    let mut tokens: Vec<i64> = vec![];
    let _ = tx.send(Cmd::Refresh);
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Refresh => {
                if let Some(s) = &session {
                    for (i, tk) in tokens.drain(..).enumerate() {
                        let _ = match i {
                            0 => s.RemoveMediaPropertiesChanged(tk),
                            1 => s.RemovePlaybackInfoChanged(tk),
                            _ => s.RemoveTimelinePropertiesChanged(tk),
                        };
                    }
                }
                session = mgr.GetCurrentSession().ok();
                if let Some(s) = &session {
                    let a = tx.clone();
                    let b = tx.clone();
                    let c = tx.clone();
                    if let Ok(tk) = s.MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                        let _ = a.send(Cmd::Props);
                        Ok(())
                    })) {
                        tokens.push(tk);
                    }
                    if let Ok(tk) = s.PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                        let _ = b.send(Cmd::Playback);
                        Ok(())
                    })) {
                        tokens.push(tk);
                    }
                    if let Ok(tk) = s.TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                        let _ = c.send(Cmd::Playback);
                        Ok(())
                    })) {
                        tokens.push(tk);
                    }
                }
                info = MediaInfo { art_serial: info.art_serial, ..Default::default() };
                read_props(&session, &mut info);
                read_playback(&session, &mut info);
                publish(&info);
            }
            Cmd::Props => {
                read_props(&session, &mut info);
                read_playback(&session, &mut info);
                publish(&info);
            }
            Cmd::Playback => {
                read_playback(&session, &mut info);
                publish(&info);
            }
            Cmd::PlayPause => {
                if let Some(s) = &session {
                    if let Ok(op) = s.TryTogglePlayPauseAsync() {
                        let _ = op.join();
                    }
                }
            }
            Cmd::Next => {
                if let Some(s) = &session {
                    if let Ok(op) = s.TrySkipNextAsync() {
                        let _ = op.join();
                    }
                }
            }
            Cmd::Prev => {
                if let Some(s) = &session {
                    if let Ok(op) = s.TrySkipPreviousAsync() {
                        let _ = op.join();
                    }
                }
            }
            Cmd::Seek(sec) => {
                if let Some(s) = &session {
                    if let Ok(op) = s.TryChangePlaybackPositionAsync((sec * 10_000_000.0) as i64) {
                        let _ = op.join();
                    }
                }
            }
        }
    }
}

fn publish(info: &MediaInfo) {
    if let Ok(mut g) = SNAPSHOT.lock() {
        *g = Some(info.clone());
    }
    post(WM_APP_MEDIA, 0, 0);
}

fn read_props(session: &Option<GlobalSystemMediaTransportControlsSession>, info: &mut MediaInfo) {
    let Some(s) = session else {
        info.has_session = false;
        return;
    };
    info.has_session = true;
    info.app_id = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
    let Ok(op) = s.TryGetMediaPropertiesAsync() else { return };
    let Ok(p) = op.join() else { return };
    let title = p.Title().map(|h| h.to_string()).unwrap_or_default();
    let artist = p.Artist().map(|h| h.to_string()).unwrap_or_default();
    let album = p.AlbumTitle().map(|h| h.to_string()).unwrap_or_default();
    let changed = title != info.title || artist != info.artist;
    info.title = title;
    info.artist = artist;
    info.album = album;
    if let Ok(th) = p.Thumbnail() {
        if let Some(bytes) = read_stream(&th) {
            let same = info.art.as_ref().map(|a| a.len() == bytes.len() && **a == bytes).unwrap_or(false);
            if !same {
                info.art = Some(std::sync::Arc::new(bytes));
                info.art_serial += 1;
            }
        }
    } else if changed {
        info.art = None;
        info.art_serial += 1;
    }
}

fn read_stream(r: &IRandomAccessStreamReference) -> Option<Vec<u8>> {
    let stream = r.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()? as u32;
    if size == 0 || size > 8 * 1024 * 1024 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size).ok()?.join().ok()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf).ok()?;
    Some(buf)
}

fn read_playback(session: &Option<GlobalSystemMediaTransportControlsSession>, info: &mut MediaInfo) {
    let Some(s) = session else { return };
    if let Ok(pi) = s.GetPlaybackInfo() {
        info.playing = pi.PlaybackStatus().map(|st| st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing).unwrap_or(false);
    }
    if let Ok(tl) = s.GetTimelineProperties() {
        let pos = tl.Position().map(|t| t.Duration as f64 / 1e7).unwrap_or(0.0);
        let end = tl.EndTime().map(|t| t.Duration as f64 / 1e7).unwrap_or(0.0);
        let start = tl.StartTime().map(|t| t.Duration as f64 / 1e7).unwrap_or(0.0);
        info.dur = (end - start).max(0.0);
        info.pos = (pos - start).max(0.0);
        // a posição informada vale para o instante da última atualização
        let upd = tl.LastUpdatedTime().map(|d| d.UniversalTime).unwrap_or(0);
        let now_ft = unsafe { windows::Win32::System::SystemInformation::GetSystemTimeAsFileTime() };
        let now = ((now_ft.dwHighDateTime as i64) << 32) | now_ft.dwLowDateTime as i64;
        let behind = if upd > 0 { ((now - upd) as f64 / 1e7).clamp(0.0, 3600.0) } else { 0.0 };
        if info.playing {
            info.pos = (info.pos + behind).min(info.dur.max(info.pos));
        }
        info.pos_at = crate::util::tick_ms();
    }
}

pub fn take_snapshot() -> Option<MediaInfo> {
    SNAPSHOT.lock().ok().and_then(|g| g.clone())
}

