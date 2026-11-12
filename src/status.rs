// Estado do sistema exibido na dock: volume, rede e bateria. Tudo por
// notificação do Windows (sem consulta periódica).
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Media::Audio::Endpoints::*;
use windows::Win32::Media::Audio::*;
use windows::Win32::NetworkManagement::IpHelper::*;
use windows::Win32::NetworkManagement::Ndis::*;
use windows::Win32::NetworkManagement::WiFi::*;
use windows::Win32::Networking::WinSock::AF_UNSPEC;
use windows::Win32::System::Com::*;
use windows::Win32::System::Power::*;
use windows::Win32::System::SystemServices::{GUID_ACDC_POWER_SOURCE, GUID_BATTERY_PERCENTAGE_REMAINING, GUID_POWER_SAVING_STATUS};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::{post, WM_APP_STATUS};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Net {
    None,
    Wifi(u32),
    Ethernet,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Battery {
    pub present: bool,
    pub pct: u32,
    pub charging: bool,
    pub ac: bool,
    pub secs_left: Option<u32>,
    pub saver: bool,
}

pub struct Status {
    pub vol: f32,
    pub muted: bool,
    pub has_audio: bool,
    pub net: Net,
    pub ssid: String,
    pub battery: Battery,
    endpoint: Option<IAudioEndpointVolume>,
    callback: Option<IAudioEndpointVolumeCallback>,
    enumerator: Option<IMMDeviceEnumerator>,
    notif: Option<IMMNotificationClient>,
    wlan: HANDLE,
    net_handle: HANDLE,
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolCb;

impl IAudioEndpointVolumeCallback_Impl for VolCb_Impl {
    fn OnNotify(&self, _p: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> Result<()> {
        post(WM_APP_STATUS, 1, 0);
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DevCb;

impl IMMNotificationClient_Impl for DevCb_Impl {
    fn OnDeviceStateChanged(&self, _id: &PCWSTR, _s: DEVICE_STATE) -> Result<()> {
        Ok(())
    }
    fn OnDeviceAdded(&self, _id: &PCWSTR) -> Result<()> {
        Ok(())
    }
    fn OnDeviceRemoved(&self, _id: &PCWSTR) -> Result<()> {
        Ok(())
    }
    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _id: &PCWSTR) -> Result<()> {
        if flow == eRender && role == eConsole {
            post(WM_APP_STATUS, 2, 0);
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _id: &PCWSTR, _k: &windows::Win32::Foundation::PROPERTYKEY) -> Result<()> {
        Ok(())
    }
}

unsafe extern "system" fn ip_changed(_ctx: *const core::ffi::c_void, _row: *const MIB_IPINTERFACE_ROW, _t: MIB_NOTIFICATION_TYPE) {
    post(WM_APP_STATUS, 3, 0);
}

unsafe extern "system" fn wlan_cb(data: *mut L2_NOTIFICATION_DATA, _ctx: *mut core::ffi::c_void) {
    if !data.is_null() {
        post(WM_APP_STATUS, 3, 0);
    }
}

impl Status {
    pub fn new() -> Status {
        Status {
            vol: 0.0,
            muted: false,
            has_audio: false,
            net: Net::None,
            ssid: String::new(),
            battery: Battery::default(),
            endpoint: None,
            callback: None,
            enumerator: None,
            notif: None,
            wlan: HANDLE::default(),
            net_handle: HANDLE::default(),
        }
    }

    pub fn init(&mut self, main: HWND) {
        unsafe {
            if let Ok(en) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                let n: IMMNotificationClient = DevCb.into();
                let _ = en.RegisterEndpointNotificationCallback(&n);
                self.notif = Some(n);
                self.enumerator = Some(en);
            }
            self.init_audio();
            let mut h = HANDLE::default();
            if NotifyIpInterfaceChange(AF_UNSPEC, Some(ip_changed), None, false, &mut h).is_ok() {
                self.net_handle = h;
            }
            let mut ver = 0u32;
            let mut wh = HANDLE::default();
            if WlanOpenHandle(2, None, &mut ver, &mut wh) == 0 {
                self.wlan = wh;
                let _ = WlanRegisterNotification(wh, WLAN_NOTIFICATION_SOURCE_ACM | WLAN_NOTIFICATION_SOURCE_MSM, true, Some(wlan_cb), None, None, None);
            }
            let _ = RegisterPowerSettingNotification(HANDLE(main.0), &GUID_BATTERY_PERCENTAGE_REMAINING, DEVICE_NOTIFY_WINDOW_HANDLE);
            let _ = RegisterPowerSettingNotification(HANDLE(main.0), &GUID_ACDC_POWER_SOURCE, DEVICE_NOTIFY_WINDOW_HANDLE);
            let _ = RegisterPowerSettingNotification(HANDLE(main.0), &GUID_POWER_SAVING_STATUS, DEVICE_NOTIFY_WINDOW_HANDLE);
        }
        self.refresh_net();
        self.refresh_battery();
    }

    pub fn init_audio(&mut self) {
        unsafe {
            if let (Some(ep), Some(cb)) = (&self.endpoint, &self.callback) {
                let _ = ep.UnregisterControlChangeNotify(cb);
            }
            self.endpoint = None;
            self.has_audio = false;
            let Some(en) = &self.enumerator else { return };
            let Ok(dev) = en.GetDefaultAudioEndpoint(eRender, eConsole) else { return };
            let Ok(ep) = dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) else { return };
            let cb: IAudioEndpointVolumeCallback = VolCb.into();
            let _ = ep.RegisterControlChangeNotify(&cb);
            self.callback = Some(cb);
            self.endpoint = Some(ep);
            self.has_audio = true;
        }
        self.refresh_volume();
    }

    pub fn refresh_volume(&mut self) {
        if let Some(ep) = &self.endpoint {
            unsafe {
                self.vol = ep.GetMasterVolumeLevelScalar().unwrap_or(0.0);
                self.muted = ep.GetMute().map(|b| b.as_bool()).unwrap_or(false);
            }
        }
    }

    pub fn change_volume(&mut self, delta: f32) {
        if let Some(ep) = &self.endpoint {
            unsafe {
                let v = (self.vol + delta).clamp(0.0, 1.0);
                let _ = ep.SetMasterVolumeLevelScalar(v, std::ptr::null());
                if self.muted && delta > 0.0 {
                    let _ = ep.SetMute(false, std::ptr::null());
                }
            }
            self.refresh_volume();
        }
    }

    pub fn set_volume(&mut self, v: f32) {
        if let Some(ep) = &self.endpoint {
            unsafe {
                let _ = ep.SetMasterVolumeLevelScalar(v.clamp(0.0, 1.0), std::ptr::null());
                if self.muted && v > 0.0 {
                    let _ = ep.SetMute(false, std::ptr::null());
                }
            }
            self.refresh_volume();
        }
    }

        pub fn toggle_mute(&mut self) {
        if let Some(ep) = &self.endpoint {
            unsafe {
                let _ = ep.SetMute(!self.muted, std::ptr::null());
            }
            self.refresh_volume();
        }
    }

    pub fn refresh_net(&mut self) {
        self.net = Net::None;
        self.ssid.clear();
        unsafe {
            let mut size = 16 * 1024u32;
            let mut buf: Vec<u8> = vec![0; size as usize];
            let mut r = GetAdaptersAddresses(0, GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER, None, Some(buf.as_mut_ptr() as _), &mut size);
            if r == ERROR_BUFFER_OVERFLOW.0 {
                buf = vec![0; size as usize];
                r = GetAdaptersAddresses(0, GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER, None, Some(buf.as_mut_ptr() as _), &mut size);
            }
            if r != 0 {
                return;
            }
            let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
            let mut wifi = false;
            let mut eth = false;
            while !p.is_null() {
                let a = &*p;
                if a.OperStatus == IfOperStatusUp && !a.FirstGatewayAddress.is_null() {
                    if a.IfType == IF_TYPE_IEEE80211 {
                        wifi = true;
                    } else if a.IfType != IF_TYPE_SOFTWARE_LOOPBACK && a.IfType != IF_TYPE_TUNNEL {
                        eth = true;
                    }
                }
                p = a.Next;
            }
            if wifi {
                let (q, ssid) = self.wifi_quality();
                self.net = Net::Wifi(q);
                self.ssid = ssid;
            } else if eth {
                self.net = Net::Ethernet;
            }
        }
    }

    fn wifi_quality(&self) -> (u32, String) {
        if self.wlan.is_invalid() {
            return (100, String::new());
        }
        unsafe {
            let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
            if WlanEnumInterfaces(self.wlan, None, &mut list) != 0 || list.is_null() {
                return (100, String::new());
            }
            let n = (*list).dwNumberOfItems as usize;
            let items = std::slice::from_raw_parts((*list).InterfaceInfo.as_ptr(), n);
            let mut best = (0u32, String::new());
            for it in items {
                if it.isState != wlan_interface_state_connected {
                    continue;
                }
                let mut size = 0u32;
                let mut data: *mut core::ffi::c_void = std::ptr::null_mut();
                if WlanQueryInterface(self.wlan, &it.InterfaceGuid, wlan_intf_opcode_current_connection, None, &mut size, &mut data, None) == 0
                    && !data.is_null()
                {
                    let attr = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
                    let q = attr.wlanAssociationAttributes.wlanSignalQuality;
                    let s = &attr.wlanAssociationAttributes.dot11Ssid;
                    let ssid = String::from_utf8_lossy(&s.ucSSID[..s.uSSIDLength as usize]).to_string();
                    if q >= best.0 {
                        best = (q, ssid);
                    }
                    WlanFreeMemory(data);
                }
            }
            WlanFreeMemory(list as _);
            best
        }
    }

    pub fn refresh_battery(&mut self) {
        unsafe {
            let mut ps = SYSTEM_POWER_STATUS::default();
            if GetSystemPowerStatus(&mut ps).is_err() {
                return;
            }
            let present = ps.BatteryFlag & 128 == 0 && ps.BatteryFlag != 255 && ps.BatteryLifePercent <= 100;
            self.battery = Battery {
                present,
                pct: ps.BatteryLifePercent.min(100) as u32,
                charging: ps.BatteryFlag & 8 != 0,
                ac: ps.ACLineStatus == 1,
                secs_left: if ps.BatteryLifeTime != u32::MAX && ps.BatteryLifeTime > 0 { Some(ps.BatteryLifeTime) } else { None },
                saver: ps.SystemStatusFlag == 1,
            };
        }
    }

    /// Ícone (Segoe Fluent) de rede.
    pub fn net_glyph(&self) -> u32 {
        match self.net {
            Net::None => 0xF384,
            Net::Ethernet => 0xE839,
            Net::Wifi(q) => {
                if q >= 75 {
                    0xE701
                } else if q >= 50 {
                    0xE874
                } else if q >= 25 {
                    0xE873
                } else {
                    0xE872
                }
            }
        }
    }

    pub fn vol_glyph(&self) -> u32 {
        if self.muted || self.vol <= 0.001 {
            0xE74F
        } else if self.vol < 0.34 {
            0xE993
        } else if self.vol < 0.67 {
            0xE994
        } else {
            0xE995
        }
    }

    pub fn battery_glyph(&self) -> u32 {
        let b = &self.battery;
        let lvl = ((b.pct as f32 / 10.0).round() as u32).min(10);
        if b.charging || b.ac {
            if lvl >= 10 {
                0xE83E
            } else {
                0xE85A + lvl
            }
        } else if lvl >= 10 {
            0xE83F
        } else {
            0xE850 + lvl
        }
    }

    pub fn shutdown(&mut self) {
        unsafe {
            if let (Some(ep), Some(cb)) = (&self.endpoint, &self.callback) {
                let _ = ep.UnregisterControlChangeNotify(cb);
            }
            if let (Some(en), Some(n)) = (&self.enumerator, &self.notif) {
                let _ = en.UnregisterEndpointNotificationCallback(n);
            }
            if !self.net_handle.is_invalid() {
                let _ = CancelMibChangeNotify2(self.net_handle);
            }
            if !self.wlan.is_invalid() {
                let _ = WlanCloseHandle(self.wlan, None);
            }
        }
    }
}

pub fn power_msg_is_battery(wp: WPARAM) -> bool {
    wp.0 as u32 == PBT_POWERSETTINGCHANGE || wp.0 as u32 == PBT_APMPOWERSTATUSCHANGE
}
