// Processador, memória e tráfego de rede. Só é amostrado quando algum
// widget visível precisa (o temporizador desliga sozinho).
use std::collections::VecDeque;
use windows::Win32::Foundation::*;
use windows::Win32::NetworkManagement::IpHelper::*;
use windows::Win32::System::SystemInformation::*;
use windows::Win32::System::Threading::GetSystemTimes;

const HIST: usize = 60;

#[derive(Clone, Debug)]
pub struct IfaceInfo {
    pub index: u32,
    pub name: String,
}

pub struct SysMon {
    prev_cpu: Option<(u64, u64, u64)>,
    prev_net: Option<(u64, u64, u64)>,
    pub cpu: f32,
    pub mem: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub cpu_hist: VecDeque<f32>,
    pub mem_hist: VecDeque<f32>,
    pub net_hist: VecDeque<(f64, f64)>,
    pub iface: u32,
    pub ifaces: Vec<IfaceInfo>,
    pub samples: u64,
}

fn ft(f: &FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

impl SysMon {
    pub fn new() -> SysMon {
        SysMon {
            prev_cpu: None,
            prev_net: None,
            cpu: 0.0,
            mem: 0.0,
            mem_used: 0,
            mem_total: 0,
            rx_bps: 0.0,
            tx_bps: 0.0,
            cpu_hist: VecDeque::with_capacity(HIST),
            mem_hist: VecDeque::with_capacity(HIST),
            net_hist: VecDeque::with_capacity(HIST),
            iface: 0,
            ifaces: vec![],
            samples: 0,
        }
    }

    pub fn sample(&mut self) {
        unsafe {
            let (mut i, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
            if GetSystemTimes(Some(&mut i), Some(&mut k), Some(&mut u)).is_ok() {
                let cur = (ft(&i), ft(&k), ft(&u));
                if let Some(p) = self.prev_cpu {
                    let idle = cur.0.saturating_sub(p.0) as f64;
                    let total = (cur.1.saturating_sub(p.1) + cur.2.saturating_sub(p.2)) as f64;
                    if total > 0.0 {
                        self.cpu = (((total - idle) / total).clamp(0.0, 1.0)) as f32;
                    }
                }
                self.prev_cpu = Some(cur);
            }
            let mut ms = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
            if GlobalMemoryStatusEx(&mut ms).is_ok() {
                self.mem_total = ms.ullTotalPhys;
                self.mem_used = ms.ullTotalPhys - ms.ullAvailPhys;
                self.mem = ms.dwMemoryLoad as f32 / 100.0;
            }
            self.sample_net();
        }
        push(&mut self.cpu_hist, self.cpu);
        push(&mut self.mem_hist, self.mem);
        if self.net_hist.len() >= HIST {
            self.net_hist.pop_front();
        }
        self.net_hist.push_back((self.rx_bps, self.tx_bps));
        self.samples += 1;
    }

    unsafe fn sample_net(&mut self) {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return;
        }
        let n = (*table).NumEntries as usize;
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), n);
        let (mut rx, mut tx) = (0u64, 0u64);
        let mut ifaces = vec![];
        for r in rows {
            let hw = r.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
            if !hw || r.OperStatus != windows::Win32::NetworkManagement::Ndis::IfOperStatusUp {
                continue;
            }
            if r.Type == 24 {
                continue;
            }
            let name = String::from_utf16_lossy(&r.Alias).trim_end_matches('\0').to_string();
            ifaces.push(IfaceInfo { index: r.InterfaceIndex, name });
            if self.iface == 0 || self.iface == r.InterfaceIndex {
                rx += r.InOctets;
                tx += r.OutOctets;
            }
        }
        FreeMibTable(table as _);
        self.ifaces = ifaces;
        let now = crate::util::tick_ms();
        if let Some((prx, ptx, pt)) = self.prev_net {
            let dt = (now.saturating_sub(pt)) as f64 / 1000.0;
            if dt > 0.05 {
                self.rx_bps = rx.saturating_sub(prx) as f64 / dt;
                self.tx_bps = tx.saturating_sub(ptx) as f64 / dt;
            }
        }
        self.prev_net = Some((rx, tx, now));
    }
}

fn push(q: &mut VecDeque<f32>, v: f32) {
    if q.len() >= HIST {
        q.pop_front();
    }
    q.push_back(v);
}
