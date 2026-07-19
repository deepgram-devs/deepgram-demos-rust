use std::ffi::OsStr;
use std::fs;
use std::iter;
use std::mem::ManuallyDrop;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, NO_ERROR, PROPERTYKEY, RPC_E_CHANGED_MODE, WIN32_ERROR,
};
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0, PropVariantClear,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemAlloc, CoUninitialize, IPersistFile,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ, REG_VALUE_TYPE,
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{IShellLinkW, SetCurrentProcessExplicitAppUserModelID, ShellLink};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;
use windows::core::{GUID, Interface, PWSTR};

const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_APPROVED_RUN_KEY_PATH: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const STARTUP_APPROVED_FOLDER_KEY_PATH: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder";
const VALUE_NAME: &str = "Deepgram Velocity";
const LEGACY_VALUE_NAME: &str = "Velocity";
const SHORTCUT_NAME: &str = "Deepgram Velocity.lnk";
const ICON_BYTES: &[u8] = include_bytes!("../assets/deepgram-icon.ico");
pub const APP_USER_MODEL_ID: &str = "Deepgram.Velocity";
pub const START_MINIMIZED_ARG: &str = "--start-minimized";

const PKEY_APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};

pub fn set_current_process_app_user_model_id() -> Result<(), String> {
    let app_id = to_wide(APP_USER_MODEL_ID);
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(PCWSTR(app_id.as_ptr()))
            .map_err(|error| format!("Failed to set Velocity AppUserModelID: {error}"))
    }
}

pub fn is_enabled() -> Result<bool, String> {
    Ok(startup_shortcut_enabled()?
        || startup_run_value_enabled(VALUE_NAME)?
        || startup_run_value_enabled(LEGACY_VALUE_NAME)?)
}

pub fn enable() -> Result<(), String> {
    create_startup_shortcut()?;
    delete_legacy_startup_registry_values()?;
    delete_registry_value(STARTUP_APPROVED_FOLDER_KEY_PATH, SHORTCUT_NAME)?;
    Ok(())
}

pub fn disable() -> Result<(), String> {
    delete_startup_shortcut()?;
    delete_startup_registry_values()?;
    Ok(())
}

pub fn repair_if_enabled() -> Result<(), String> {
    if is_enabled()? {
        create_startup_shortcut()?;
        delete_legacy_startup_registry_values()?;
    }

    Ok(())
}

fn delete_startup_registry_values() -> Result<(), String> {
    delete_legacy_startup_registry_values()?;
    delete_registry_value(STARTUP_APPROVED_FOLDER_KEY_PATH, SHORTCUT_NAME)
}

fn delete_legacy_startup_registry_values() -> Result<(), String> {
    delete_registry_value(RUN_KEY_PATH, VALUE_NAME)?;
    delete_registry_value(RUN_KEY_PATH, LEGACY_VALUE_NAME)?;
    delete_registry_value(STARTUP_APPROVED_RUN_KEY_PATH, VALUE_NAME)?;
    delete_registry_value(STARTUP_APPROVED_RUN_KEY_PATH, LEGACY_VALUE_NAME)
}

fn create_startup_shortcut() -> Result<(), String> {
    let exe = current_exe()?;
    let icon = write_startup_icon()?;
    let link_path = shortcut_path()?;
    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create Windows Startup folder: {error}"))?;
    }

    let _com = ComApartment::init()?;
    let link: IShellLinkW = unsafe {
        CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| format!("Failed to create Windows shortcut object: {error}"))?
    };

    let exe_text = exe.to_string_lossy();
    let exe_wide = to_wide(&exe_text);
    let icon_text = icon.to_string_lossy();
    let icon_wide = to_wide(&icon_text);
    let args_wide = to_wide(START_MINIMIZED_ARG);
    let description = to_wide("Deepgram Velocity");

    unsafe {
        link.SetPath(PCWSTR(exe_wide.as_ptr()))
            .map_err(|error| format!("Failed to set startup shortcut target: {error}"))?;
        link.SetArguments(PCWSTR(args_wide.as_ptr()))
            .map_err(|error| format!("Failed to set startup shortcut arguments: {error}"))?;
        if let Some(parent) = exe.parent() {
            let working_directory = to_wide(&parent.to_string_lossy());
            link.SetWorkingDirectory(PCWSTR(working_directory.as_ptr()))
                .map_err(|error| {
                    format!("Failed to set startup shortcut working directory: {error}")
                })?;
        }
        link.SetDescription(PCWSTR(description.as_ptr()))
            .map_err(|error| format!("Failed to set startup shortcut description: {error}"))?;
        link.SetIconLocation(PCWSTR(icon_wide.as_ptr()), 0)
            .map_err(|error| format!("Failed to set startup shortcut icon: {error}"))?;
        link.SetShowCmd(SW_SHOWNORMAL)
            .map_err(|error| format!("Failed to set startup shortcut window state: {error}"))?;
    }

    set_shortcut_app_user_model_id(&link)?;

    if link_path.exists() {
        fs::remove_file(&link_path).map_err(|error| {
            format!(
                "Failed to refresh startup shortcut {}: {error}",
                link_path.display()
            )
        })?;
    }

    let persist: IPersistFile = link
        .cast()
        .map_err(|error| format!("Failed to prepare startup shortcut save: {error}"))?;
    let link_path_text = link_path.to_string_lossy();
    let link_path_wide = to_wide(&link_path_text);
    unsafe {
        persist
            .Save(PCWSTR(link_path_wide.as_ptr()), true)
            .map_err(|error| format!("Failed to save startup shortcut: {error}"))?;
    }

    Ok(())
}

fn set_shortcut_app_user_model_id(link: &IShellLinkW) -> Result<(), String> {
    let property_store: IPropertyStore = link
        .cast()
        .map_err(|error| format!("Failed to prepare startup shortcut identity: {error}"))?;
    let prop_variant = PropVariantString::new(APP_USER_MODEL_ID)?;

    unsafe {
        property_store
            .SetValue(&PKEY_APP_USER_MODEL_ID, prop_variant.as_ptr())
            .map_err(|error| format!("Failed to set startup shortcut identity: {error}"))?;
        property_store
            .Commit()
            .map_err(|error| format!("Failed to save startup shortcut identity: {error}"))?;
    }

    Ok(())
}

struct PropVariantString {
    value: PROPVARIANT,
}

impl PropVariantString {
    fn new(value: &str) -> Result<Self, String> {
        let wide = to_wide(value);
        let byte_len = wide
            .len()
            .checked_mul(std::mem::size_of::<u16>())
            .ok_or_else(|| "AppUserModelID is too large".to_string())?;
        let allocated = unsafe { CoTaskMemAlloc(byte_len) as *mut u16 };
        if allocated.is_null() {
            return Err(
                "Failed to allocate AppUserModelID string for shortcut identity".to_string(),
            );
        }

        unsafe {
            std::ptr::copy_nonoverlapping(wide.as_ptr(), allocated, wide.len());
        }

        Ok(Self {
            value: PROPVARIANT {
                Anonymous: PROPVARIANT_0 {
                    Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                        vt: VT_LPWSTR,
                        wReserved1: 0,
                        wReserved2: 0,
                        wReserved3: 0,
                        Anonymous: PROPVARIANT_0_0_0 {
                            pwszVal: PWSTR(allocated),
                        },
                    }),
                },
            },
        })
    }

    fn as_ptr(&self) -> *const PROPVARIANT {
        &self.value
    }
}

impl Drop for PropVariantString {
    fn drop(&mut self) {
        unsafe {
            let _ = PropVariantClear(&mut self.value);
        }
    }
}

fn write_startup_icon() -> Result<PathBuf, String> {
    let icon = startup_support_dir()?.join(startup_icon_file_name());
    if let Some(parent) = icon.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create startup icon folder: {error}"))?;
    }

    if fs::read(&icon).map_or(false, |current| current == ICON_BYTES) {
        return Ok(icon);
    }

    fs::write(&icon, ICON_BYTES)
        .map_err(|error| format!("Failed to write startup icon {}: {error}", icon.display()))?;

    Ok(icon)
}

fn startup_icon_file_name() -> String {
    format!("deepgram-velocity-{}.ico", env!("CARGO_PKG_VERSION"))
}

fn delete_startup_shortcut() -> Result<(), String> {
    let path = shortcut_path()?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Failed to delete {}: {error}", path.display()))?;
    }

    Ok(())
}

fn delete_registry_value(key_path: &str, value_name: &str) -> Result<(), String> {
    let key = match open_key(key_path, KEY_SET_VALUE) {
        Ok(key) => key,
        Err(error) if error.contains("code 2") => return Ok(()),
        Err(error) => return Err(error),
    };
    let value_name = to_wide(value_name);
    let result = unsafe { RegDeleteValueW(key.raw(), PCWSTR(value_name.as_ptr())) };
    drop(key);

    if result == NO_ERROR || result == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(format_registry_error(
            "delete Windows startup entry",
            result,
        ))
    }
}

fn current_exe() -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|error| format!("Failed to resolve the Velocity executable path: {error}"))
}

fn shortcut_path() -> Result<PathBuf, String> {
    Ok(startup_folder()?.join(SHORTCUT_NAME))
}

fn startup_folder() -> Result<PathBuf, String> {
    let appdata = std::env::var_os("APPDATA").ok_or_else(|| {
        "APPDATA is not set; cannot locate the Windows Startup folder".to_string()
    })?;

    Ok(PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup"))
}

fn startup_support_dir() -> Result<PathBuf, String> {
    let appdata = std::env::var_os("APPDATA").ok_or_else(|| {
        "APPDATA is not set; cannot locate the Velocity startup support folder".to_string()
    })?;

    Ok(PathBuf::from(appdata).join("Deepgram").join("Velocity"))
}

fn startup_shortcut_enabled() -> Result<bool, String> {
    Ok(shortcut_path()?.exists()
        && !startup_approved_value_disabled(STARTUP_APPROVED_FOLDER_KEY_PATH, SHORTCUT_NAME)?)
}

fn startup_run_value_enabled(name: &str) -> Result<bool, String> {
    Ok(read_named_run_value(name)?.is_some()
        && !startup_approved_value_disabled(STARTUP_APPROVED_RUN_KEY_PATH, name)?)
}

fn startup_approved_value_disabled(key_path: &str, name: &str) -> Result<bool, String> {
    Ok(
        read_registry_value_bytes(key_path, name)?.and_then(|bytes| bytes.first().copied())
            == Some(3),
    )
}

fn read_named_run_value(name: &str) -> Result<Option<String>, String> {
    let key = match open_key(RUN_KEY_PATH, KEY_QUERY_VALUE) {
        Ok(key) => key,
        Err(error) if error.contains("code 2") => return Ok(None),
        Err(error) => return Err(error),
    };

    let value_name = to_wide(name);
    let mut value_type = REG_VALUE_TYPE::default();
    let mut byte_count = 0u32;
    let size_result = unsafe {
        RegQueryValueExW(
            key.raw(),
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut value_type),
            None,
            Some(&mut byte_count),
        )
    };

    if size_result == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if size_result != NO_ERROR {
        return Err(format_registry_error(
            "read Windows startup entry",
            size_result,
        ));
    }
    if value_type != REG_SZ || byte_count == 0 {
        return Ok(None);
    }

    let mut buffer = vec![0u16; byte_count.div_ceil(2) as usize];
    let read_result = unsafe {
        RegQueryValueExW(
            key.raw(),
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut value_type),
            Some(buffer.as_mut_ptr() as *mut u8),
            Some(&mut byte_count),
        )
    };

    if read_result != NO_ERROR {
        return Err(format_registry_error(
            "read Windows startup entry",
            read_result,
        ));
    }

    Ok(Some(
        String::from_utf16_lossy(&buffer)
            .trim_end_matches('\0')
            .to_string(),
    ))
}

fn read_registry_value_bytes(key_path: &str, name: &str) -> Result<Option<Vec<u8>>, String> {
    let key = match open_key(key_path, KEY_QUERY_VALUE) {
        Ok(key) => key,
        Err(error) if error.contains("code 2") => return Ok(None),
        Err(error) => return Err(error),
    };

    let value_name = to_wide(name);
    let mut value_type = REG_VALUE_TYPE::default();
    let mut byte_count = 0u32;
    let size_result = unsafe {
        RegQueryValueExW(
            key.raw(),
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut value_type),
            None,
            Some(&mut byte_count),
        )
    };

    if size_result == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if size_result != NO_ERROR {
        return Err(format_registry_error(
            "read Windows startup approval entry",
            size_result,
        ));
    }

    let mut buffer = vec![0u8; byte_count as usize];
    let read_result = unsafe {
        RegQueryValueExW(
            key.raw(),
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut value_type),
            Some(buffer.as_mut_ptr()),
            Some(&mut byte_count),
        )
    };

    if read_result != NO_ERROR {
        return Err(format_registry_error(
            "read Windows startup approval entry",
            read_result,
        ));
    }

    buffer.truncate(byte_count as usize);
    Ok(Some(buffer))
}

fn open_key(key_path: &str, access: REG_SAM_FLAGS) -> Result<RegistryKey, String> {
    let subkey = to_wide(key_path);
    let mut key = HKEY::default();
    let result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            None,
            access,
            &mut key,
        )
    };

    if result == NO_ERROR {
        Ok(RegistryKey(key))
    } else {
        Err(format_registry_error(
            "open Windows startup registry bookkeeping key",
            result,
        ))
    }
}

fn format_registry_error(action: &str, error: WIN32_ERROR) -> String {
    format!("Failed to {action}: Windows error code {}", error.0)
}

fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(iter::once(0))
        .collect()
}

struct RegistryKey(HKEY);

impl RegistryKey {
    fn raw(&self) -> HKEY {
        self.0
    }
}

impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

struct ComApartment {
    should_uninitialize: bool,
}

impl ComApartment {
    fn init() -> Result<Self, String> {
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result == RPC_E_CHANGED_MODE {
            return Ok(Self {
                should_uninitialize: false,
            });
        }
        if result.is_err() {
            return Err(format!(
                "Failed to initialize COM for startup shortcut creation: HRESULT 0x{:08X}",
                result.0 as u32
            ));
        }

        Ok(Self {
            should_uninitialize: true,
        })
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.should_uninitialize {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_uses_start_minimized_argument() {
        assert_eq!(START_MINIMIZED_ARG, "--start-minimized");
    }

    #[test]
    fn startup_app_name_is_branded() {
        assert_eq!(APP_USER_MODEL_ID, "Deepgram.Velocity");
        assert_eq!(VALUE_NAME, "Deepgram Velocity");
        assert_eq!(LEGACY_VALUE_NAME, "Velocity");
        assert_eq!(SHORTCUT_NAME, "Deepgram Velocity.lnk");
        assert_eq!(startup_icon_file_name(), "deepgram-velocity-0.5.1.ico");
    }
}
