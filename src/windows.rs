#![cfg(target_os = "windows")]

use crate::errors::MIDError;
use crate::windows_smbios::parse_smbios_mid;
use std::{ffi::c_void, io, mem, ptr};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    BusType1394, BusTypeMmc, BusTypeSd, BusTypeUsb, CreateFileW, FILE_ATTRIBUTE_NORMAL,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::{
    IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery, STORAGE_DEVICE_DESCRIPTOR,
    STORAGE_PROPERTY_QUERY, StorageDeviceProperty,
};
use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows_sys::Win32::System::SystemInformation::{GetSystemFirmwareTable, RSMB};

const PLACEHOLDER_BIOS_SERIAL: &str = "system serial number";
const MAX_PHYSICAL_DRIVES: u8 = 16;

pub(crate) fn get_mid_result() -> Result<String, MIDError> {
    let firmware_table = read_raw_smbios_table()?;
    let combined_string = parse_smbios_mid(&firmware_table)?;

    if combined_string.is_empty() {
        return Err(MIDError::ResultMidError);
    }

    if uses_placeholder_bios_serial(&combined_string) {
        let replacement = get_machine_guid_disk_mid();
        if !replacement.is_empty() {
            return Ok(replacement);
        }
    }

    Ok(combined_string)
}

fn read_raw_smbios_table() -> Result<Vec<u8>, MIDError> {
    let table_size = unsafe { GetSystemFirmwareTable(RSMB, 0, ptr::null_mut(), 0) };

    if table_size == 0 {
        return Err(MIDError::ReadSystemDataError(io::Error::last_os_error()));
    }

    let mut buffer = vec![0u8; table_size as usize];
    let bytes_written =
        unsafe { GetSystemFirmwareTable(RSMB, 0, buffer.as_mut_ptr().cast(), table_size) };

    if bytes_written == 0 {
        return Err(MIDError::ReadSystemDataError(io::Error::last_os_error()));
    }

    if bytes_written != table_size {
        return Err(MIDError::InvalidSystemData(format!(
            "expected {table_size} bytes, got {bytes_written} bytes"
        )));
    }

    Ok(buffer)
}

fn uses_placeholder_bios_serial(mid: &str) -> bool {
    mid.split('|')
        .nth(1)
        .is_some_and(|serial| serial.trim().eq_ignore_ascii_case(PLACEHOLDER_BIOS_SERIAL))
}

fn get_machine_guid_disk_mid() -> String {
    format_machine_guid_disk_mid(
        read_machine_guid().as_deref(),
        read_internal_disk_serial().as_deref(),
    )
}

fn format_machine_guid_disk_mid(machine_guid: Option<&str>, disk_serial: Option<&str>) -> String {
    let parts: Vec<String> = [machine_guid, disk_serial]
        .into_iter()
        .flatten()
        .map(normalize_mid_part)
        .filter(|value| !value.is_empty())
        .collect();

    parts.join("|")
}

fn read_machine_guid() -> Option<String> {
    let subkey = wide_null("SOFTWARE\\Microsoft\\Cryptography");
    let value_name = wide_null("MachineGuid");
    let mut buffer = vec![0u16; 128];
    let mut byte_len = (buffer.len() * mem::size_of::<u16>()) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            buffer.as_mut_ptr().cast::<c_void>(),
            &mut byte_len,
        )
    };

    if status != ERROR_SUCCESS {
        return None;
    }

    string_from_wide_null(&buffer).map(|value| normalize_mid_part(&value))
}

fn read_internal_disk_serial() -> Option<String> {
    (0..MAX_PHYSICAL_DRIVES).find_map(read_physical_drive_serial)
}

fn read_physical_drive_serial(index: u8) -> Option<String> {
    let path = wide_null(&format!("\\\\.\\PhysicalDrive{index}"));
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return None;
    }

    let handle = Handle(handle);
    query_storage_device_descriptor(handle.0).and_then(|buffer| {
        let descriptor = unsafe { &*(buffer.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>()) };
        if descriptor.RemovableMedia || is_external_bus(descriptor.BusType) {
            return None;
        }

        read_descriptor_string(&buffer, descriptor.SerialNumberOffset)
            .map(|value| normalize_mid_part(&value))
    })
}

fn query_storage_device_descriptor(handle: HANDLE) -> Option<Vec<u8>> {
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut buffer = vec![0u8; 4096];
    let mut bytes_returned = 0u32;

    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&query as *const STORAGE_PROPERTY_QUERY).cast::<c_void>(),
            mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            buffer.as_mut_ptr().cast::<c_void>(),
            buffer.len() as u32,
            &mut bytes_returned,
            ptr::null_mut(),
        )
    };

    if ok == 0 || bytes_returned < mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() as u32 {
        return None;
    }

    Some(buffer)
}

fn is_external_bus(bus_type: i32) -> bool {
    bus_type == BusTypeUsb
        || bus_type == BusTypeSd
        || bus_type == BusTypeMmc
        || bus_type == BusType1394
}

fn read_descriptor_string(buffer: &[u8], offset: u32) -> Option<String> {
    let offset = offset as usize;
    if offset == 0 || offset >= buffer.len() {
        return None;
    }

    let bytes = &buffer[offset..];
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec()).ok()
}

fn normalize_mid_part(value: &str) -> String {
    value.trim().to_lowercase()
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

fn string_from_wide_null(value: &[u16]) -> Option<String> {
    let end = value.iter().position(|ch| *ch == 0).unwrap_or(value.len());
    if end == 0 {
        return None;
    }

    String::from_utf16(&value[..end]).ok()
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_placeholder_bios_serial() {
        assert!(uses_placeholder_bios_serial(
            "uuid|System Serial Number|board|processor"
        ));
        assert!(uses_placeholder_bios_serial(
            "uuid| system serial number |board|processor"
        ));
        assert!(!uses_placeholder_bios_serial(
            "uuid|real-serial|board|processor"
        ));
        assert!(!uses_placeholder_bios_serial("uuid"));
    }

    #[test]
    fn formats_machine_guid_disk_mid() {
        assert_eq!(
            format_machine_guid_disk_mid(
                Some("90b53d31-d95d-4e30-956d-50adb4bee030"),
                Some("0000_0000_0000_0000_0026_B778_5710_D8D5")
            ),
            "90b53d31-d95d-4e30-956d-50adb4bee030|0000_0000_0000_0000_0026_b778_5710_d8d5"
        );
    }

    #[test]
    fn uses_partial_machine_guid_disk_mid() {
        assert_eq!(format_machine_guid_disk_mid(Some("guid"), None), "guid");
        assert_eq!(format_machine_guid_disk_mid(None, Some("disk")), "disk");
        assert_eq!(format_machine_guid_disk_mid(None, None), "");
    }

    #[test]
    fn reads_descriptor_string() {
        let mut buffer = vec![0u8; 32];
        buffer[8..19].copy_from_slice(b"DISK-SERIAL");

        assert_eq!(read_descriptor_string(&buffer, 8).unwrap(), "DISK-SERIAL");
        assert!(read_descriptor_string(&buffer, 0).is_none());
        assert!(read_descriptor_string(&buffer, 32).is_none());
    }
}
