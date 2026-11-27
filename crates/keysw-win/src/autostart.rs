//! Autostart through Task Scheduler.
//!
//! A logon task with "run with highest privileges" starts KeySW elevated
//! without a UAC prompt at every logon. Elevation matters: Windows does not
//! let a normal process see typing in windows that run as administrator.
//!
//! The exe is copied to %LOCALAPPDATA%\Programs\KeySW so the task keeps
//! working if the downloaded file is moved or deleted.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, WaitForSingleObject, INFINITE,
};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

pub const TASK_NAME: &str = "KeySW";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

pub fn is_enabled() -> bool {
    Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn install_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Programs").join("KeySW")
}

/// Copy the running exe into the install dir (unless it already runs from
/// there) and return the installed path.
fn install_copy() -> std::io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    let dir = install_dir();
    let target = dir.join("keysw-win.exe");
    let same = std::fs::canonicalize(&current).ok() == std::fs::canonicalize(&target).ok();
    if !same {
        std::fs::create_dir_all(&dir)?;
        std::fs::copy(&current, &target)?;
    }
    Ok(target)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn task_xml(exe: &Path) -> String {
    let user = format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap_or_default(),
        std::env::var("USERNAME").unwrap_or_default()
    );
    let user = xml_escape(&user);
    let exe = xml_escape(&exe.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>KeySW: automatic keyboard layout switching</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// Run a program elevated (UAC prompt) and wait for it. Ok(exit code), or
/// Err if the person declined the prompt.
pub fn run_elevated(file: &str, params: &str) -> Result<u32, ()> {
    let file = HSTRING::from(file);
    let params = HSTRING::from(params);
    unsafe {
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        ShellExecuteExW(&mut info).map_err(|_| ())?;
        if info.hProcess.is_invalid() {
            return Ok(0);
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 1u32;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        Ok(code)
    }
}

/// Install the exe and register the logon task (asks for admin rights once).
pub fn enable() -> Result<(), String> {
    let exe = install_copy().map_err(|e| format!("не вдалося скопіювати програму: {e}"))?;
    let xml_path = std::env::temp_dir().join("keysw-task.xml");
    // Task Scheduler expects UTF-16 with a byte-order mark.
    let mut bytes = vec![0xFF, 0xFE];
    for u in task_xml(&exe).encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    std::fs::write(&xml_path, bytes).map_err(|e| e.to_string())?;
    let params = format!("/Create /TN {TASK_NAME} /XML \"{}\" /F", xml_path.display());
    let result = run_elevated("schtasks.exe", &params);
    let _ = std::fs::remove_file(&xml_path);
    match result {
        Ok(0) => Ok(()),
        Ok(code) => Err(format!("schtasks завершився з кодом {code}")),
        Err(()) => Err("потрібен дозвіл адміністратора".into()),
    }
}

pub fn disable() -> Result<(), String> {
    match run_elevated("schtasks.exe", &format!("/Delete /TN {TASK_NAME} /F")) {
        Ok(0) => Ok(()),
        Ok(code) => Err(format!("schtasks завершився з кодом {code}")),
        Err(()) => Err("потрібен дозвіл адміністратора".into()),
    }
}

/// Start the installed copy through the task (elevated, no UAC prompt).
pub fn start_task() -> bool {
    Command::new("schtasks")
        .args(["/Run", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
