
#[cfg(windows)]
pub fn get_current_exe_path() -> Option<String> {
    use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
    let mut buf = [0u16; 1024];
    // SAFETY: Retrieves the full path of the executable for the current process.
    let len = unsafe { GetModuleFileNameW(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) };
    if len > 0 && len < buf.len() as u32 {
        String::from_utf16(&buf[..len as usize]).ok()
    } else {
        std::env::current_exe().ok().and_then(|p| p.to_str().map(|s| s.to_owned()))
    }
}

#[cfg(windows)]
pub fn set_autostart_registry(enable: bool) {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE,
        REG_SZ,
    };

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0".encode_utf16().collect();
    let val_name: Vec<u16> = "NothingNotes\0".encode_utf16().collect();

    let mut hkey = std::ptr::null_mut();
    // SAFETY: Opens the Windows Run key in HKEY_CURRENT_USER.
    let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_SET_VALUE, &mut hkey) };
    if status != 0 {
        return;
    }

    if enable {
        if let Some(exe) = get_current_exe_path() {
            let formatted = format!("\"{exe}\"\0");
            let exe_utf16: Vec<u16> = formatted.encode_utf16().collect();
            // SAFETY: Sets the registry string value pointing to the current executable path.
            unsafe {
                RegSetValueExW(
                    hkey,
                    val_name.as_ptr(),
                    0,
                    REG_SZ,
                    exe_utf16.as_ptr() as *const u8,
                    (exe_utf16.len() * 2) as u32,
                );
            }
        }
    } else {
        // SAFETY: Deletes the Run registry entry if disabled.
        unsafe {
            RegDeleteValueW(hkey, val_name.as_ptr());
        }
    }

    // SAFETY: Closes the opened registry key handle.
    unsafe {
        RegCloseKey(hkey);
    }
}

#[cfg(not(windows))]
pub fn set_autostart_registry(_enable: bool) {}