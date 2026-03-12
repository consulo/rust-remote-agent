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

use crate::generated::remote_agent::{SystemInfo, UserInfo};

pub fn get_env_variable(name: &str) -> String {
    env::var(name).unwrap_or_default()
}

pub fn get_env_variables() -> BTreeMap<String, String> {
    env::vars().collect()
}

pub fn get_system_info() -> SystemInfo {
    SystemInfo::new(
        env::consts::OS.to_string(),
        os_version(),
        env::consts::ARCH.to_string(),
        hostname(),
        num_cpus(),
        total_memory(),
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

fn os_version() -> String {
    #[cfg(windows)]
    {
        env::var("OS").unwrap_or_else(|_| "Windows".into())
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/version")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "Linux".into())
    }
    #[cfg(target_os = "macos")]
    {
        "macOS".into()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        "unknown".into()
    }
}

fn num_cpus() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(1)
}

fn total_memory() -> i64 {
    #[cfg(windows)]
    {
        // Use GlobalMemoryStatusEx via raw WinAPI
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
                status.total_phys as i64
            } else {
                0
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|contents| {
                contents
                    .lines()
                    .find(|line| line.starts_with("MemTotal:"))
                    .and_then(|line| {
                        line.split_whitespace()
                            .nth(1)
                            .and_then(|v| v.parse::<i64>().ok())
                    })
                    .map(|kb| kb * 1024)
            })
            .unwrap_or(0)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        0
    }
}
