// System status for the Control Centre and the assistant's tools: CPU load,
// memory, disks and battery. Read on request only, nothing is polled in the
// background, and nothing leaves the computer.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
    pub mount: String,
    pub total: u64,
    pub available: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub os: String,
    pub cpu_name: String,
    pub cpus: u32,
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_available: u64,
    pub groovy_memory: u64,
    pub disks: Vec<Disk>,
    pub battery: Option<Battery>,
    pub uptime_secs: u64,
}

#[tauri::command]
pub async fn system_status() -> SystemStatus {
    tauri::async_runtime::spawn_blocking(read).await.unwrap_or_else(|_| read())
}

/// The status now (blocks ~0.2 s for the CPU sample).
pub fn read_now() -> SystemStatus {
    read()
}

fn read() -> SystemStatus {
    use sysinfo::{Disks, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_usage();
    // CPU usage is a difference between two samples.
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();
    let pid = sysinfo::get_current_pid().ok();
    if let Some(pid) = pid {
        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    }
    let disks = Disks::new_with_refreshed_list()
        .list()
        .iter()
        .map(|d| Disk { mount: d.mount_point().to_string_lossy().to_string(), total: d.total_space(), available: d.available_space() })
        .filter(|d| d.total > 0)
        .collect();
    SystemStatus {
        os: System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string()),
        cpu_name: sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        cpus: sys.cpus().len() as u32,
        cpu_usage: sys.global_cpu_usage(),
        memory_total: sys.total_memory(),
        memory_available: sys.available_memory(),
        groovy_memory: pid.and_then(|p| sys.process(p)).map(|p| p.memory()).unwrap_or(0),
        disks,
        battery: battery(),
        uptime_secs: System::uptime(),
    }
}

#[cfg(windows)]
fn battery() -> Option<Battery> {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s).ok()? };
    // 128 = no battery, 255 = unknown.
    if s.BatteryFlag & 128 != 0 || s.BatteryLifePercent > 100 {
        return None;
    }
    Some(Battery { percent: s.BatteryLifePercent, charging: s.ACLineStatus == 1 })
}

#[cfg(not(windows))]
fn battery() -> Option<Battery> {
    let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for e in dir.flatten() {
        let p = e.path();
        if std::fs::read_to_string(p.join("type")).ok()?.trim() != "Battery" {
            continue;
        }
        let percent: u8 = std::fs::read_to_string(p.join("capacity")).ok()?.trim().parse().ok()?;
        let status = std::fs::read_to_string(p.join("status")).unwrap_or_default();
        return Some(Battery { percent: percent.min(100), charging: status.trim() == "Charging" || status.trim() == "Full" });
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_something_sensible() {
        let s = super::read();
        assert!(s.cpus >= 1);
        assert!(s.memory_total > 0);
        assert!(s.memory_available <= s.memory_total);
        assert!((0.0..=100.0 * s.cpus as f32).contains(&s.cpu_usage));
    }
}
