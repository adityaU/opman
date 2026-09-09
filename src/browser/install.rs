use super::binary::{
    BrowserInstallCommand, BrowserInstallGuide, BrowserSetupIssue, BROWSER_BIN_ENV,
};

/// Common install guidance for the host that owns the browser process.
pub fn install_guide() -> BrowserInstallGuide {
    if cfg!(target_os = "linux") {
        return BrowserInstallGuide {
            platform: "linux",
            title: "Install Chromium on Linux",
            summary:
                "Install a Chromium-based browser on the machine running opman, then restart opman.",
            steps: &[
                "Choose the command for your Linux distribution.",
                "Restart opman so it can discover the new browser.",
            ],
            commands: &[
                BrowserInstallCommand {
                    label: "Debian / Ubuntu",
                    command: "sudo apt update && sudo apt install -y chromium",
                },
                BrowserInstallCommand {
                    label: "Fedora",
                    command: "sudo dnf install -y chromium",
                },
                BrowserInstallCommand {
                    label: "Arch Linux",
                    command: "sudo pacman -S chromium",
                },
                BrowserInstallCommand {
                    label: "openSUSE",
                    command: "sudo zypper install chromium",
                },
            ],
            env_var: BROWSER_BIN_ENV,
            docs_url: "https://www.chromium.org/getting-involved/download-chromium/",
            issue: BrowserSetupIssue::NoBrowser,
        };
    }

    if cfg!(target_os = "macos") {
        return BrowserInstallGuide {
            platform: "macos",
            title: "Install Chrome or Chromium on macOS",
            summary:
                "Install a Chromium-based browser on the Mac running opman, then restart opman.",
            steps: &[
                "Install Google Chrome with Homebrew, or install Chromium from the download page.",
                "Restart opman so it can discover the new browser.",
            ],
            commands: &[BrowserInstallCommand {
                label: "Homebrew",
                command: "brew install --cask google-chrome",
            }],
            env_var: BROWSER_BIN_ENV,
            docs_url: "https://www.chromium.org/getting-involved/download-chromium/",
            issue: BrowserSetupIssue::NoBrowser,
        };
    }

    if cfg!(target_os = "windows") {
        return BrowserInstallGuide {
            platform: "windows",
            title: "Install Chrome or Chromium on Windows",
            summary: "Install a Chromium-based browser on the Windows machine running opman, then restart opman.",
            steps: &[
                "Install Google Chrome with winget, or install Chromium from the download page.",
                "Restart opman so it can discover the new browser.",
            ],
            commands: &[
                BrowserInstallCommand {
                    label: "PowerShell",
                    command: "winget install --id Google.Chrome -e",
                },
                BrowserInstallCommand {
                    label: "Set the browser path if needed",
                    command: "[Environment]::SetEnvironmentVariable(\"OPMAN_BROWSER_BIN\", \"$env:ProgramFiles\\Google\\Chrome\\Application\\chrome.exe\", \"User\")",
                },
            ],
            env_var: BROWSER_BIN_ENV,
            docs_url: "https://www.chromium.org/getting-involved/download-chromium/",
            issue: BrowserSetupIssue::NoBrowser,
        };
    }

    BrowserInstallGuide {
        platform: "other",
        title: "Install a Chromium-based browser",
        summary:
            "Install Chromium or Google Chrome on the machine running opman, then restart opman.",
        steps: &[
            "Install a Chromium-based browser using your operating system's package manager.",
            "Make sure its executable is on PATH, or set the browser path below.",
            "Restart opman so it can discover the new browser.",
        ],
        commands: &[],
        env_var: BROWSER_BIN_ENV,
        docs_url: "https://www.chromium.org/getting-involved/download-chromium/",
        issue: BrowserSetupIssue::NoBrowser,
    }
}
