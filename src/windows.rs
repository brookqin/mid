#![cfg(target_os = "windows")]

use crate::errors::MIDError;
use crate::utils::run_shell_command;

pub(crate) fn get_mid_result() -> Result<String, MIDError> {
    let combined_output = run_shell_command(
        "powershell",
        [
            "-WindowStyle",
            "Hidden",
            "-command",
            r#"
            $csproduct = Get-WmiObject Win32_ComputerSystemProduct | Select-Object -ExpandProperty UUID;
            $bios = Get-WmiObject Win32_BIOS | Select-Object -ExpandProperty SerialNumber;
            $baseboard = Get-WmiObject Win32_BaseBoard | Select-Object -ExpandProperty SerialNumber;
            $cpu = Get-WmiObject Win32_Processor | Select-Object -ExpandProperty ProcessorId;
            "$csproduct|$bios|$baseboard|$cpu"
            "#,
        ],
    )
    .unwrap_or(String::new());

    if combined_output.is_empty() {
        return Err(MIDError::ResultMidError);
    }

    Ok(normalize_output(&combined_output))
}

fn normalize_output(output: &str) -> String {
    output
        .trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::normalize_output;

    #[test]
    fn normalize_output_trims_outer_delimiters_and_lowercases() {
        assert_eq!(
            normalize_output(" |UUID-123|BIOS-456|BOARD-789|CPU-ABC| \r\n"),
            "uuid-123|bios-456|board-789|cpu-abc"
        );
    }
}
