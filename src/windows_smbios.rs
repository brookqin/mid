use crate::errors::MIDError;

const RAW_SMBIOS_HEADER_LEN: usize = 8;
const SMBIOS_TYPE_SYSTEM: u8 = 1;
const SMBIOS_TYPE_BASEBOARD: u8 = 2;
const SMBIOS_TYPE_PROCESSOR: u8 = 4;
const SMBIOS_TYPE_END_OF_TABLE: u8 = 127;

#[derive(Default)]
struct WindowsMidFields {
    system_uuid: Option<String>,
    system_serial: Option<String>,
    baseboard_serial: Option<String>,
    processor_id: Option<String>,
}

pub(crate) fn parse_smbios_mid(raw_table: &[u8]) -> Result<String, MIDError> {
    if raw_table.len() < RAW_SMBIOS_HEADER_LEN {
        return Err(MIDError::InvalidSystemData(
            "raw SMBIOS data is shorter than its header".to_string(),
        ));
    }

    let table_len =
        u32::from_le_bytes([raw_table[4], raw_table[5], raw_table[6], raw_table[7]]) as usize;
    let table_start = RAW_SMBIOS_HEADER_LEN;
    let table_end = table_start
        .checked_add(table_len)
        .ok_or_else(|| MIDError::InvalidSystemData("SMBIOS length overflow".to_string()))?;

    if table_end > raw_table.len() {
        return Err(MIDError::InvalidSystemData(format!(
            "SMBIOS table length {table_len} exceeds buffer length {}",
            raw_table.len() - RAW_SMBIOS_HEADER_LEN
        )));
    }

    let mut fields = WindowsMidFields::default();
    let mut offset = table_start;

    while offset < table_end {
        let structure = parse_structure(raw_table, offset, table_end)?;
        if structure.kind == SMBIOS_TYPE_END_OF_TABLE {
            break;
        }

        match structure.kind {
            SMBIOS_TYPE_SYSTEM => {
                if fields.system_uuid.is_none() {
                    fields.system_uuid = extract_system_uuid(structure.formatted);
                }
                if fields.system_serial.is_none() {
                    fields.system_serial =
                        extract_string_field(structure.formatted, 7, &structure.strings);
                }
            }
            SMBIOS_TYPE_BASEBOARD => {
                if fields.baseboard_serial.is_none() {
                    fields.baseboard_serial =
                        extract_string_field(structure.formatted, 7, &structure.strings);
                }
            }
            SMBIOS_TYPE_PROCESSOR => {
                if fields.processor_id.is_none() {
                    fields.processor_id = extract_processor_id(structure.formatted);
                }
            }
            _ => {}
        }

        offset = structure.next_offset;
    }

    let result: Vec<String> = [
        fields.system_uuid,
        fields.system_serial,
        fields.baseboard_serial,
        fields.processor_id,
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.is_empty())
    .collect();

    Ok(result.join("|").to_lowercase())
}

struct SmbiosStructure<'a> {
    kind: u8,
    formatted: &'a [u8],
    strings: Vec<String>,
    next_offset: usize,
}

fn parse_structure(
    raw_table: &[u8],
    offset: usize,
    table_end: usize,
) -> Result<SmbiosStructure<'_>, MIDError> {
    if offset + 4 > table_end {
        return Err(MIDError::InvalidSystemData(
            "truncated SMBIOS structure header".to_string(),
        ));
    }

    let kind = raw_table[offset];
    let length = raw_table[offset + 1] as usize;

    if length < 4 {
        return Err(MIDError::InvalidSystemData(format!(
            "SMBIOS structure type {kind} has invalid length {length}"
        )));
    }

    let formatted_end = offset.checked_add(length).ok_or_else(|| {
        MIDError::InvalidSystemData("SMBIOS structure length overflow".to_string())
    })?;

    if formatted_end > table_end {
        return Err(MIDError::InvalidSystemData(format!(
            "SMBIOS structure type {kind} exceeds table length"
        )));
    }

    let strings_end = find_strings_end(raw_table, formatted_end, table_end)?;
    let strings = parse_strings(&raw_table[formatted_end..strings_end]);

    Ok(SmbiosStructure {
        kind,
        formatted: &raw_table[offset..formatted_end],
        strings,
        next_offset: strings_end + 2,
    })
}

fn find_strings_end(
    raw_table: &[u8],
    mut offset: usize,
    table_end: usize,
) -> Result<usize, MIDError> {
    while offset + 1 < table_end {
        if raw_table[offset] == 0 && raw_table[offset + 1] == 0 {
            return Ok(offset);
        }
        offset += 1;
    }

    Err(MIDError::InvalidSystemData(
        "unterminated SMBIOS string table".to_string(),
    ))
}

fn parse_strings(raw_strings: &[u8]) -> Vec<String> {
    raw_strings
        .split(|byte| *byte == 0)
        .filter_map(|bytes| {
            let value = String::from_utf8_lossy(bytes).trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        })
        .collect()
}

fn extract_string_field(formatted: &[u8], offset: usize, strings: &[String]) -> Option<String> {
    let string_index = *formatted.get(offset)?;
    if string_index == 0 {
        return None;
    }

    strings.get((string_index - 1) as usize).cloned()
}

fn extract_system_uuid(formatted: &[u8]) -> Option<String> {
    let uuid = formatted.get(8..24)?;

    if uuid.iter().all(|byte| *byte == 0) || uuid.iter().all(|byte| *byte == 0xff) {
        return None;
    }

    Some(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        uuid[3],
        uuid[2],
        uuid[1],
        uuid[0],
        uuid[5],
        uuid[4],
        uuid[7],
        uuid[6],
        uuid[8],
        uuid[9],
        uuid[10],
        uuid[11],
        uuid[12],
        uuid[13],
        uuid[14],
        uuid[15]
    ))
}

fn extract_processor_id(formatted: &[u8]) -> Option<String> {
    let processor_id = formatted.get(8..16)?;

    if processor_id.iter().all(|byte| *byte == 0) || processor_id.iter().all(|byte| *byte == 0xff) {
        return None;
    }

    let eax = u32::from_le_bytes([
        processor_id[0],
        processor_id[1],
        processor_id[2],
        processor_id[3],
    ]);
    let edx = u32::from_le_bytes([
        processor_id[4],
        processor_id[5],
        processor_id[6],
        processor_id[7],
    ]);

    Some(format!("{edx:08x}{eax:08x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_smbios_mid_extracts_expected_windows_fields() {
        let table = raw_smbios_table(vec![
            structure(
                SMBIOS_TYPE_SYSTEM,
                &[
                    1, 2, 3, 4, 0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa,
                    0xbb, 0xcc, 0xdd, 0xee, 0xff,
                ],
                &["Acme", "Workstation", "1.0", "System-Serial"],
            ),
            structure(
                SMBIOS_TYPE_BASEBOARD,
                &[1, 2, 3, 4],
                &[
                    "BoardVendor",
                    "BoardProduct",
                    "BoardVersion",
                    "Board-Serial",
                ],
            ),
            structure(
                SMBIOS_TYPE_PROCESSOR,
                &[1, 3, 4, 5, 0xea, 0x06, 0x09, 0x00, 0xff, 0xfb, 0xeb, 0xbf],
                &["CPU0", "GenuineIntel", "Intel"],
            ),
            structure(SMBIOS_TYPE_END_OF_TABLE, &[], &[]),
        ]);

        assert_eq!(
            parse_smbios_mid(&table).unwrap(),
            "00112233-4455-6677-8899-aabbccddeeff|system-serial|board-serial|bfebfbff000906ea"
        );
    }

    #[test]
    fn parse_smbios_mid_skips_missing_and_placeholder_values() {
        let table = raw_smbios_table(vec![
            structure(
                SMBIOS_TYPE_SYSTEM,
                &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                &[],
            ),
            structure(SMBIOS_TYPE_BASEBOARD, &[0, 0, 0, 0], &[]),
            structure(
                SMBIOS_TYPE_PROCESSOR,
                &[0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
                &[],
            ),
            structure(SMBIOS_TYPE_END_OF_TABLE, &[], &[]),
        ]);

        assert_eq!(parse_smbios_mid(&table).unwrap(), "");
    }

    #[test]
    fn parse_smbios_mid_rejects_truncated_table() {
        let table = [0, 3, 8, 0, 32, 0, 0, 0, SMBIOS_TYPE_SYSTEM, 24, 0, 0];

        assert!(matches!(
            parse_smbios_mid(&table),
            Err(MIDError::InvalidSystemData(_))
        ));
    }

    fn raw_smbios_table(structures: Vec<Vec<u8>>) -> Vec<u8> {
        let table_data: Vec<u8> = structures.into_iter().flatten().collect();
        let mut raw = vec![0, 3, 8, 0];
        raw.extend_from_slice(&(table_data.len() as u32).to_le_bytes());
        raw.extend_from_slice(&table_data);
        raw
    }

    fn structure(kind: u8, payload: &[u8], strings: &[&str]) -> Vec<u8> {
        let length = 4 + payload.len();
        let mut bytes = vec![kind, length as u8, 0, 0];
        bytes.extend_from_slice(payload);

        for value in strings {
            bytes.extend_from_slice(value.as_bytes());
            bytes.push(0);
        }

        bytes.push(0);
        if strings.is_empty() {
            bytes.push(0);
        }
        bytes
    }
}
