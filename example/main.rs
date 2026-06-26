use std::env;
use std::io::{self, Write};

fn main() {
    print_header("Windows ID source debug");
    print_field("OS", env::consts::OS);
    print_field("Arch", env::consts::ARCH);
    println!();

    #[cfg(target_os = "windows")]
    print_windows_context();

    #[cfg(not(target_os = "windows"))]
    {
        print_header("Windows source diagnostics");
        println!("This example prints extra Windows fields only when built and run on Windows.");
        println!();
    }

    wait_for_enter();
}

#[cfg(target_os = "windows")]
fn print_windows_context() {
    print_header("Windows MID source fields");

    let script = r#"
function Value($object, $property) {
    if ($null -eq $object) { return "" }
    $value = $object.$property
    if ($null -eq $value) { return "" }
    return FormatValue $value
}

function RegValue($path, $name) {
    try {
        $item = Get-ItemProperty -Path $path -Name $name -ErrorAction Stop
        return Value $item $name
    } catch {
        return ""
    }
}

function FormatValue($value) {
    if ($null -eq $value) { return "" }
    if ($value -is [byte[]]) {
        return (($value | ForEach-Object { "{0:x2}" -f ([byte]$_) }) -join "")
    }
    return ([string]$value).Trim()
}

function Line($name, $value) {
    Write-Output ("{0}: {1}" -f $name, (FormatValue $value))
}

function JoinValues($values) {
    if ($null -eq $values) { return "" }
    return (($values | Where-Object { $_ } | ForEach-Object { ([string]$_).Trim() } | Sort-Object -Unique) -join ", ")
}

function JoinCertificateThumbprints($certificates) {
    if ($null -eq $certificates) { return "" }
    return (($certificates | Where-Object { $_ -and $_.Thumbprint } | ForEach-Object { $_.Thumbprint.Trim() } | Sort-Object -Unique) -join ", ")
}

function TpmEndorsementKeyInfo() {
    try {
        return Get-TpmEndorsementKeyInfo -HashAlgorithm sha256 -ErrorAction Stop
    } catch {
        return $null
    }
}

$csProduct = Get-CimInstance -ClassName Win32_ComputerSystemProduct -ErrorAction SilentlyContinue
$bios = Get-CimInstance -ClassName Win32_BIOS -ErrorAction SilentlyContinue
$baseboard = Get-CimInstance -ClassName Win32_BaseBoard -ErrorAction SilentlyContinue
$processor = Get-CimInstance -ClassName Win32_Processor -ErrorAction SilentlyContinue | Select-Object -First 1
$operatingSystem = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction SilentlyContinue
$diskDrives = Get-CimInstance -ClassName Win32_DiskDrive -ErrorAction SilentlyContinue
$physicalMedia = Get-CimInstance -ClassName Win32_PhysicalMedia -ErrorAction SilentlyContinue
$tpmEk = TpmEndorsementKeyInfo
$machineGuid = RegValue "HKLM:\SOFTWARE\Microsoft\Cryptography" "MachineGuid"

Line "✓ ComputerSystemProduct.UUID" (Value $csProduct "UUID")
Line "✓ ComputerSystemProduct.IdentifyingNumber" (Value $csProduct "IdentifyingNumber")
Line "BIOS.SerialNumber" (Value $bios "SerialNumber")
Line "✓ BaseBoard.SerialNumber" (Value $baseboard "SerialNumber")
Line "✓ Processor.ProcessorId" (Value $processor "ProcessorId")
Line "Registry.MachineGuid" $machineGuid
Line "OperatingSystem.SerialNumber" (Value $operatingSystem "SerialNumber")
Line "DiskDrive.SerialNumber" (JoinValues $diskDrives.SerialNumber)
Line "PhysicalMedia.SerialNumber" (JoinValues $physicalMedia.SerialNumber)
Line "TPM.EndorsementKey.PublicKeyHash" (Value $tpmEk "PublicKeyHash")
Line "TPM.EndorsementKey.ManufacturerCertificateThumbprint" (JoinCertificateThumbprints $tpmEk.ManufacturerCertificates)
Line "TPM.EndorsementKey.AdditionalCertificateThumbprint" (JoinCertificateThumbprints $tpmEk.AdditionalCertificates)
"#;

    match run_powershell(script) {
        Ok(output) if !output.trim().is_empty() => println!("{}", output.trim_end()),
        Ok(_) => println!("No Windows diagnostic output returned."),
        Err(error) => println!("Failed to read Windows diagnostics: {error}"),
    }

    println!();
}

#[cfg(target_os = "windows")]
fn run_powershell(script: &str) -> io::Result<String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !text.ends_with('\n') && !text.is_empty() {
            text.push('\n');
        }
        text.push_str("PowerShell stderr:\n");
        text.push_str(stderr.trim_end());
    }

    Ok(text)
}

fn print_header(title: &str) {
    println!("=== {title} ===");
}

fn print_field(name: &str, value: impl AsRef<str>) {
    println!("{name}: {}", value.as_ref());
}

fn wait_for_enter() {
    println!("Press Enter to exit...");
    let _ = io::stdout().flush();
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}
