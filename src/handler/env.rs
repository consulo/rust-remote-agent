// Copyright 2013-2026 consulo.io
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::BTreeMap;
use std::env;

use crate::generated::remote_agent::{StatInfo, SystemInfo, UserInfo};

pub fn get_env_variable(name: &str) -> String {
    env::var(name).unwrap_or_default()
}

pub fn get_env_variables() -> BTreeMap<String, String> {
    env::vars().collect()
}

pub fn get_system_info() -> SystemInfo {
    SystemInfo::new(
        os_name(),
        os_version(),
        env::consts::ARCH.to_string(),
        hostname(),
        console_encoding(),
        system_locale(),
    )
}

pub fn get_stat() -> StatInfo {
    let (total, used) = memory_stats();
    StatInfo::new(
        num_cpus(),
        thrift::OrderedFloat(cpu_load()),
        total,
        used,
    )
}

pub fn get_user_info() -> UserInfo {
    let user_name = {
        #[cfg(windows)]
        { env::var("USERNAME").unwrap_or_else(|_| "unknown".into()) }
        #[cfg(not(windows))]
        { env::var("USER").unwrap_or_else(|_| "unknown".into()) }
    };

    let home_path = {
        #[cfg(windows)]
        { env::var("USERPROFILE").unwrap_or_else(|_| "unknown".into()) }
        #[cfg(not(windows))]
        { env::var("HOME").unwrap_or_else(|_| "unknown".into()) }
    };

    UserInfo::new(user_name, home_path)
}

fn hostname() -> String {
    #[cfg(windows)]
    {
        env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".into())
    }
    #[cfg(not(windows))]
    {
        env::var("HOSTNAME")
            .or_else(|_| {
                std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|_| "unknown".into())
    }
}

/// Returns a human-readable OS name.
/// Linux: PRETTY_NAME from /etc/os-release (e.g. "Ubuntu 24.04.2 LTS")
/// macOS: "macOS <ProductVersion>" via sw_vers (e.g. "macOS 15.3")
/// Windows: "Windows" (version detail is in os_version())
fn os_name() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/etc/os-release") {
            for line in contents.lines() {
                if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
                    return value.trim_matches('"').to_string();
                }
            }
        }
        "Linux".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            if output.status.success() {
                let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return format!("macOS {}", ver);
            }
        }
        "macOS".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        "Windows".to_string()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        env::consts::OS.to_string()
    }
}

/// Returns the OS version / kernel version string.
/// Windows: "10.0.26100" via RtlGetVersion (actual build, not compatibility-shimmed)
/// Linux: kernel version from /proc/version (e.g. "6.8.0-51-generic")
/// macOS: Darwin kernel version from uname (e.g. "24.3.0")
fn os_version() -> String {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct OsVersionInfoExW {
            os_version_info_size: u32,
            major_version: u32,
            minor_version: u32,
            build_number: u32,
            platform_id: u32,
            sz_csd_version: [u16; 128],
            service_pack_major: u16,
            service_pack_minor: u16,
            suite_mask: u16,
            product_type: u8,
            reserved: u8,
        }

        unsafe extern "system" {
            fn RtlGetVersion(lp_version_information: *mut OsVersionInfoExW) -> i32;
        }

        unsafe {
            let mut info: OsVersionInfoExW = std::mem::zeroed();
            info.os_version_info_size = std::mem::size_of::<OsVersionInfoExW>() as u32;
            if RtlGetVersion(&mut info) == 0 {
                format!("{}.{}.{}", info.major_version, info.minor_version, info.build_number)
            } else {
                "unknown".to_string()
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        // Parse kernel version from /proc/version: "Linux version 6.8.0-51-generic ..."
        std::fs::read_to_string("/proc/version")
            .ok()
            .and_then(|s| {
                s.split_whitespace()
                    .nth(2)
                    .map(|v| v.to_string())
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        // Darwin kernel version via uname -r
        if let Ok(output) = std::process::Command::new("uname").arg("-r").output() {
            if output.status.success() {
                return String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }
        "unknown".to_string()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        "unknown".to_string()
    }
}

fn num_cpus() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(1)
}

/// Returns (total_memory, used_memory) in bytes.
fn memory_stats() -> (i64, i64) {
    #[cfg(windows)]
    {
        use std::mem;

        #[repr(C)]
        struct MemoryStatusEx {
            length: u32,
            memory_load: u32,
            total_phys: u64,
            avail_phys: u64,
            total_page_file: u64,
            avail_page_file: u64,
            total_virtual: u64,
            avail_virtual: u64,
            avail_extended_virtual: u64,
        }

        unsafe extern "system" {
            fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
        }

        unsafe {
            let mut status: MemoryStatusEx = mem::zeroed();
            status.length = mem::size_of::<MemoryStatusEx>() as u32;
            if GlobalMemoryStatusEx(&mut status) != 0 {
                let total = status.total_phys as i64;
                let used = total - status.avail_phys as i64;
                (total, used)
            } else {
                (0, 0)
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|contents| {
                let mut total_kb: i64 = 0;
                let mut available_kb: i64 = 0;
                for line in contents.lines() {
                    if line.starts_with("MemTotal:") {
                        total_kb = line.split_whitespace().nth(1)
                            .and_then(|v| v.parse().ok()).unwrap_or(0);
                    } else if line.starts_with("MemAvailable:") {
                        available_kb = line.split_whitespace().nth(1)
                            .and_then(|v| v.parse().ok()).unwrap_or(0);
                    }
                }
                if total_kb > 0 {
                    Some((total_kb * 1024, (total_kb - available_kb) * 1024))
                } else {
                    None
                }
            })
            .unwrap_or((0, 0))
    }
    #[cfg(target_os = "macos")]
    {
        // sysctl hw.memsize for total, vm_stat for used
        let total = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<i64>().ok())
            .unwrap_or(0);

        // vm_stat reports pages; page size is typically 16384 on Apple Silicon, 4096 on Intel
        let page_size = std::process::Command::new("sysctl")
            .args(["-n", "hw.pagesize"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<i64>().ok())
            .unwrap_or(4096);

        let used = std::process::Command::new("vm_stat")
            .output()
            .ok()
            .map(|o| {
                let text = String::from_utf8_lossy(&o.stdout);
                let mut active: i64 = 0;
                let mut wired: i64 = 0;
                let mut compressed: i64 = 0;
                for line in text.lines() {
                    let val = || -> Option<i64> {
                        line.split(':').nth(1)?.trim().trim_end_matches('.').parse().ok()
                    };
                    if line.starts_with("Pages active") { active = val().unwrap_or(0); }
                    else if line.starts_with("Pages wired") { wired = val().unwrap_or(0); }
                    else if line.starts_with("Pages occupied by compressor") { compressed = val().unwrap_or(0); }
                }
                (active + wired + compressed) * page_size
            })
            .unwrap_or(0);

        (total, used)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        (0, 0)
    }
}

/// Returns CPU load as 0.0–1.0 by sampling /proc/stat over a short interval.
fn cpu_load() -> f64 {
    #[cfg(target_os = "linux")]
    {
        fn read_cpu_times() -> Option<(u64, u64)> {
            let contents = std::fs::read_to_string("/proc/stat").ok()?;
            let line = contents.lines().find(|l| l.starts_with("cpu "))?;
            let vals: Vec<u64> = line.split_whitespace().skip(1)
                .filter_map(|v| v.parse().ok()).collect();
            if vals.len() < 4 { return None; }
            let idle = vals[3];
            let total: u64 = vals.iter().sum();
            Some((total, idle))
        }

        if let (Some((t1, i1)), Some((t2, i2))) = (
            read_cpu_times(),
            {
                std::thread::sleep(std::time::Duration::from_millis(200));
                read_cpu_times()
            },
        ) {
            let dt = t2.saturating_sub(t1);
            let di = i2.saturating_sub(i1);
            if dt > 0 { (dt - di) as f64 / dt as f64 } else { 0.0 }
        } else {
            0.0
        }
    }
    #[cfg(target_os = "macos")]
    {
        // Use sysctl kern.cp_time (not available on all versions) or top
        // Simplest: parse `sysctl -n vm.loadavg` and normalize by CPU count
        let load1 = std::process::Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                // format: "{ 1.23 4.56 7.89 }"
                s.trim().trim_start_matches('{').trim().split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<f64>().ok())
            })
            .unwrap_or(0.0);
        let cpus = num_cpus() as f64;
        (load1 / cpus).min(1.0)
    }
    #[cfg(windows)]
    {
        // Simple approach: two snapshots of GetSystemTimes
        #[repr(C)]
        #[derive(Copy, Clone)]
        struct FileTime { low: u32, high: u32 }
        impl FileTime {
            fn to_u64(self) -> u64 { (self.high as u64) << 32 | self.low as u64 }
        }

        unsafe extern "system" {
            fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        }

        unsafe {
            let (mut idle1, mut kern1, mut user1) = (std::mem::zeroed(), std::mem::zeroed(), std::mem::zeroed());
            let (mut idle2, mut kern2, mut user2) = (std::mem::zeroed(), std::mem::zeroed(), std::mem::zeroed());

            if GetSystemTimes(&mut idle1, &mut kern1, &mut user1) == 0 { return 0.0; }
            std::thread::sleep(std::time::Duration::from_millis(200));
            if GetSystemTimes(&mut idle2, &mut kern2, &mut user2) == 0 { return 0.0; }

            let idle_d = idle2.to_u64() - idle1.to_u64();
            let total_d = (kern2.to_u64() + user2.to_u64()) - (kern1.to_u64() + user1.to_u64());
            if total_d > 0 { (total_d - idle_d) as f64 / total_d as f64 } else { 0.0 }
        }
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        0.0
    }
}

/// Detect console output encoding.
/// Windows: reads OEM codepage via GetConsoleOutputCP().
/// Unix: always UTF-8 (modern default).
fn console_encoding() -> String {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetConsoleOutputCP() -> u32;
        }
        let cp = unsafe { GetConsoleOutputCP() };
        codepage_to_charset(cp).to_string()
    }
    #[cfg(not(windows))]
    {
        "UTF-8".to_string()
    }
}

#[cfg(windows)]
fn codepage_to_charset(cp: u32) -> &'static str {
    match cp {
        437 => "CP437",
        850 => "CP850",
        866 => "CP866",
        874 => "x-windows-874",
        932 => "Shift_JIS",
        936 => "GBK",
        949 => "EUC-KR",
        950 => "Big5",
        1250 => "windows-1250",
        1251 => "windows-1251",
        1252 => "windows-1252",
        1253 => "windows-1253",
        1254 => "windows-1254",
        1255 => "windows-1255",
        1256 => "windows-1256",
        1257 => "windows-1257",
        1258 => "windows-1258",
        65001 => "UTF-8",
        _ => "UTF-8",
    }
}

/// Detect system locale.
/// Unix: reads LC_ALL / LC_CTYPE / LANG environment variables.
/// Windows: reads GetUserDefaultLocaleName().
fn system_locale() -> String {
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        unsafe extern "system" {
            fn GetUserDefaultLocaleName(lp_locale_name: *mut u16, cch_locale_name: i32) -> i32;
        }

        let mut buf = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH
        let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
        if len > 0 {
            let os = OsString::from_wide(&buf[..(len - 1) as usize]);
            os.to_string_lossy().into_owned()
        } else {
            "unknown".to_string()
        }
    }
    #[cfg(not(windows))]
    {
        env::var("LC_ALL")
            .or_else(|_| env::var("LC_CTYPE"))
            .or_else(|_| env::var("LANG"))
            .unwrap_or_else(|_| "en_US.UTF-8".to_string())
    }
}
