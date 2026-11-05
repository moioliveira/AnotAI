//! Implementações Win32.

use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::core::PWSTR;

use crate::{Elevation, ForegroundApp};

/// Fecha o handle ao sair do escopo.
struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: o handle foi aberto por nós e é fechado uma única vez.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(crate) fn current_process_elevation() -> Elevation {
    // SAFETY: pseudo-handle do processo atual; não precisa ser fechado.
    elevation_of(unsafe { GetCurrentProcess() })
}

pub(crate) fn foreground_app() -> Option<ForegroundApp> {
    // SAFETY: consultas sem ponteiros compartilhados.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
    }
    let mut pid = 0u32;
    // SAFETY: `pid` é uma variável local válida.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    // SAFETY: abre o processo só para consulta limitada.
    let process = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => OwnedHandle(h),
        Err(_) => {
            return Some(ForegroundApp {
                exe: None,
                elevation: Elevation::Unknown,
            });
        }
    };
    Some(ForegroundApp {
        exe: exe_name(process.0),
        elevation: elevation_of(process.0),
    })
}

fn elevation_of(process: HANDLE) -> Elevation {
    let mut token = HANDLE::default();
    // SAFETY: `token` recebe um handle novo que fechamos logo abaixo.
    if let Err(e) = unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } {
        // Processo comum não pode ler o token de um processo elevado.
        return if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
            Elevation::Elevated
        } else {
            Elevation::Unknown
        };
    }
    let token = OwnedHandle(token);

    let mut info = TOKEN_ELEVATION::default();
    let mut len = 0u32;
    // SAFETY: `info` tem o tamanho informado.
    let ok = unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some(std::ptr::from_mut(&mut info).cast()),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
    };
    match ok {
        Ok(()) if info.TokenIsElevated != 0 => Elevation::Elevated,
        Ok(()) => Elevation::Normal,
        Err(_) => Elevation::Unknown,
    }
}

fn exe_name(process: HANDLE) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` tem `len` posições.
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    }
    .ok()?;
    let path = String::from_utf16_lossy(buf.get(..len as usize)?);
    Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_uppercase())
}
