//! Windows backend: Toolhelp snapshots + ReadProcessMemory/WriteProcessMemory.

use std::ffi::c_void;
use std::mem;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
    MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};

/// Owns the Win32 process handle; closed on drop.
pub struct ProcHandle {
    handle: HANDLE,
}

impl Drop for ProcHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// Open the process for memory access.
pub fn open(pid: u32) -> Result<ProcHandle, String> {
    let handle = unsafe {
        OpenProcess(PROCESS_ALL_ACCESS, false, pid)
            .map_err(|e| format!("OpenProcess failed: {e}"))?
    };
    Ok(ProcHandle { handle })
}

/// Read `n` bytes from the target process at `addr`.
pub fn read_bytes(h: &ProcHandle, addr: u64, n: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; n];
    let mut bytes_read = 0usize;
    unsafe {
        ReadProcessMemory(
            h.handle,
            addr as *const c_void,
            buf.as_mut_ptr() as *mut c_void,
            n,
            Some(&mut bytes_read),
        )
        .map_err(|e| format!("ReadProcessMemory at 0x{addr:X}: {e}"))?;
    }
    Ok(buf)
}

/// Write bytes to the target process at `addr`.
pub fn write_bytes(h: &ProcHandle, addr: u64, data: &[u8]) -> Result<(), String> {
    let mut bytes_written = 0usize;
    unsafe {
        WriteProcessMemory(
            h.handle,
            addr as *const c_void,
            data.as_ptr() as *const c_void,
            data.len(),
            Some(&mut bytes_written),
        )
        .map_err(|e| format!("WriteProcessMemory at 0x{addr:X}: {e}"))?;
    }
    Ok(())
}

/// Find a process by name, return its PID.
pub fn find_pid(name: &str) -> Result<u32, String> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| format!("CreateToolhelp32Snapshot: {e}"))?;

        let mut entry = PROCESSENTRY32W {
            dwSize: mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        let target = name.to_lowercase();
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let exe = String::from_utf16_lossy(
                    &entry.szExeFile[..entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len())],
                );
                if exe.to_lowercase() == target {
                    let _ = CloseHandle(snap);
                    return Ok(entry.th32ProcessID);
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        Err(format!("{name} not found"))
    }
}

/// Find a module's base address, size, and full on-disk path within a process.
pub fn find_module_info(pid: u32, module_name: &str) -> Result<(u64, u64, String), String> {
    unsafe {
        let snap =
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)
                .map_err(|e| format!("CreateToolhelp32Snapshot (module): {e}"))?;

        let mut entry = MODULEENTRY32W {
            dwSize: mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };

        let target = module_name.to_lowercase();
        if Module32FirstW(snap, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szModule[..entry
                        .szModule
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szModule.len())],
                );
                if name.to_lowercase() == target {
                    let path = String::from_utf16_lossy(
                        &entry.szExePath[..entry
                            .szExePath
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry.szExePath.len())],
                    );
                    let _ = CloseHandle(snap);
                    return Ok((entry.modBaseAddr as u64, entry.modBaseSize as u64, path));
                }
                if Module32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        Err(format!("{module_name} not found in pid {pid}"))
    }
}
