use std::env;
use std::io::{self, Write};

fn main() {
    print_header("Windows ID source debug");
    print_field("OS", env::consts::OS);
    print_field("Arch", env::consts::ARCH);
    print_field("Current exe", current_exe());
    println!();

    print_basic_context();

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

fn print_basic_context() {
    print_header("Process context");
    print_field("COMPUTERNAME", env_var("COMPUTERNAME"));
    print_field("HOSTNAME", env_var("HOSTNAME"));
    print_field("USERDOMAIN", env_var("USERDOMAIN"));
    print_field("USERNAME", env_var("USERNAME"));
    print_field("USERPROFILE", env_var("USERPROFILE"));
    println!();
}

#[cfg(target_os = "windows")]
fn print_windows_context() {
    print_header("Windows system fields");

    let script = r#"
function Value($object, $property) {
    if ($null -eq $object) { return "" }
    $value = $object.$property
    if ($null -eq $value) { return "" }
    return ([string]$value).Trim()
}

function RegValue($path, $name) {
    try {
        $item = Get-ItemProperty -Path $path -Name $name -ErrorAction Stop
        return Value $item $name
    } catch {
        return ""
    }
}

function Line($name, $value) {
    Write-Output ("{0}: {1}" -f $name, $value)
}

$csProduct = Get-CimInstance -ClassName Win32_ComputerSystemProduct -ErrorAction SilentlyContinue
$bios = Get-CimInstance -ClassName Win32_BIOS -ErrorAction SilentlyContinue
$baseboard = Get-CimInstance -ClassName Win32_BaseBoard -ErrorAction SilentlyContinue
$processor = Get-CimInstance -ClassName Win32_Processor -ErrorAction SilentlyContinue | Select-Object -First 1
$computerSystem = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction SilentlyContinue
$operatingSystem = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction SilentlyContinue
$activeComputerName = RegValue "HKLM:\SYSTEM\CurrentControlSet\Control\ComputerName\ActiveComputerName" "ComputerName"
$computerName = RegValue "HKLM:\SYSTEM\CurrentControlSet\Control\ComputerName\ComputerName" "ComputerName"
$machineGuid = RegValue "HKLM:\SOFTWARE\Microsoft\Cryptography" "MachineGuid"
$productId = RegValue "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion" "ProductId"
$installDate = RegValue "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion" "InstallDate"
$installationType = RegValue "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion" "InstallationType"

Line "ComputerSystemProduct.UUID" (Value $csProduct "UUID")
Line "ComputerSystemProduct.IdentifyingNumber" (Value $csProduct "IdentifyingNumber")
Line "ComputerSystemProduct.Vendor" (Value $csProduct "Vendor")
Line "ComputerSystemProduct.Name" (Value $csProduct "Name")
Line "BIOS.SerialNumber" (Value $bios "SerialNumber")
Line "BIOS.SMBIOSBIOSVersion" (Value $bios "SMBIOSBIOSVersion")
Line "BIOS.Manufacturer" (Value $bios "Manufacturer")
Line "BIOS.ReleaseDate" (Value $bios "ReleaseDate")
Line "BaseBoard.SerialNumber" (Value $baseboard "SerialNumber")
Line "BaseBoard.Manufacturer" (Value $baseboard "Manufacturer")
Line "BaseBoard.Product" (Value $baseboard "Product")
Line "BaseBoard.Version" (Value $baseboard "Version")
Line "Processor.ProcessorId" (Value $processor "ProcessorId")
Line "Processor.Name" (Value $processor "Name")
Line "Processor.Manufacturer" (Value $processor "Manufacturer")
Line "ComputerSystem.Manufacturer" (Value $computerSystem "Manufacturer")
Line "ComputerSystem.Model" (Value $computerSystem "Model")
Line "ComputerSystem.Domain" (Value $computerSystem "Domain")
Line "ComputerSystem.PartOfDomain" (Value $computerSystem "PartOfDomain")
Line "ComputerSystem.PrimaryOwnerName" (Value $computerSystem "PrimaryOwnerName")
Line "OperatingSystem.Caption" (Value $operatingSystem "Caption")
Line "OperatingSystem.Version" (Value $operatingSystem "Version")
Line "OperatingSystem.SerialNumber" (Value $operatingSystem "SerialNumber")
Line "Registry.ActiveComputerName" $activeComputerName
Line "Registry.ComputerName" $computerName
Line "Registry.MachineGuid" $machineGuid
Line "Registry.WindowsProductId" $productId
Line "Registry.WindowsInstallDate" $installDate
Line "Registry.InstallationType" $installationType

Write-Output ""
Write-Output "Network adapters:"
$adapters = Get-CimInstance -ClassName Win32_NetworkAdapterConfiguration -Filter "IPEnabled = True" -ErrorAction SilentlyContinue
foreach ($adapter in $adapters) {
    $ips = ""
    if ($null -ne $adapter.IPAddress) {
        $ips = ($adapter.IPAddress -join ", ")
    }
    Write-Output ("  Description: {0}" -f $adapter.Description)
    Write-Output ("  MACAddress: {0}" -f $adapter.MACAddress)
    Write-Output ("  DHCPEnabled: {0}" -f $adapter.DHCPEnabled)
    Write-Output ("  IPAddress: {0}" -f $ips)
    Write-Output ""
}
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

fn env_var(name: &str) -> String {
    env::var(name).unwrap_or_default()
}

fn current_exe() -> String {
    env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_default()
}

fn wait_for_enter() {
    println!("Press Enter to exit...");
    let _ = io::stdout().flush();
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}
