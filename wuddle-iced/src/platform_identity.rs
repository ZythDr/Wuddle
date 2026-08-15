#[cfg(target_os = "linux")]
pub const LINUX_APPLICATION_ID: &str = "wuddle";

#[cfg(target_os = "windows")]
pub fn initialize() {
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    let application_id = "ZythDr.Wuddle\0".encode_utf16().collect::<Vec<_>>();
    let result = unsafe { SetCurrentProcessExplicitAppUserModelID(application_id.as_ptr()) };
    if result < 0 {
        eprintln!(
            "Wuddle could not set its Windows application identity (HRESULT {result:#010x})."
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn initialize() {}
