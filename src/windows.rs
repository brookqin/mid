#![cfg(target_os = "windows")]

use crate::errors::MIDError;
use crate::windows_smbios::parse_smbios_mid;
use std::io;
use std::ptr;
use windows_sys::Win32::System::SystemInformation::{GetSystemFirmwareTable, RSMB};

pub(crate) fn get_mid_result() -> Result<String, MIDError> {
    let firmware_table = read_raw_smbios_table()?;
    let combined_string = parse_smbios_mid(&firmware_table)?;

    if combined_string.is_empty() {
        return Err(MIDError::ResultMidError);
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
