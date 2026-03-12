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

/// Detect if running inside Windows Subsystem for Linux.
#[cfg(target_os = "linux")]
pub fn is_wsl() -> bool {
    // Check /proc/version for "microsoft" or "WSL" (case-insensitive)
    if let Ok(version) = std::fs::read_to_string("/proc/version") {
        let lower = version.to_lowercase();
        return lower.contains("microsoft") || lower.contains("wsl");
    }
    false
}

#[cfg(not(target_os = "linux"))]
pub fn is_wsl() -> bool {
    false
}

/// Human-readable platform label for logging/diagnostics.
pub fn platform_label() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        if is_wsl() {
            "linux-wsl"
        } else {
            "linux"
        }
    } else {
        // Covers Haiku, FreeBSD, etc.
        std::env::consts::OS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_label_not_empty() {
        let label = platform_label();
        assert!(!label.is_empty());
    }

    #[test]
    fn test_is_wsl_returns_bool() {
        // Just ensure it doesn't panic
        let _ = is_wsl();
    }
}
