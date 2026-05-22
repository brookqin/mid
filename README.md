# mid

A lightweight Rust crate for generating machine ID hashes on Linux, macOS, and Windows.

This project is a simplified fork of [doroved/mid](https://github.com/doroved/mid). Mobile platform code, build scripts, examples, and documentation have been removed so the crate only focuses on desktop platforms.

## Supported Platforms

- Linux
- macOS
- Windows

iOS, Android, and other mobile targets are not supported.

## Features

- `mid::get(key)`: returns the HMAC-SHA256 hash of the machine ID data.
- `mid::data(key)`: returns the raw platform fields and the final hash.
- `mid::print(key)`: prints debug information only in debug builds.
- `mid::additional_data()`: macOS only, returns extra device information that is not used in the hash.

The `key` must not be empty. Machine ID values can change after hardware replacement, OS reinstall, virtualization changes, or changes to the underlying system fields.

## Installation

```toml
[dependencies]
mid = "5.0.1"
```

Enable serialization support when needed:

```toml
[dependencies]
mid = { version = "5.0.1", features = ["serde", "serde_json"] }
```

## Usage

Get the machine ID hash:

```rust
let machine_id = mid::get("mySecretKey")?;
println!("{machine_id}");
```

Get the raw fields and hash:

```rust
let data = mid::data("mySecretKey")?;
println!("{:?}", data.result);
println!("{}", data.hash);
```

Get additional macOS device data:

```rust
#[cfg(target_os = "macos")]
{
    let info = mid::additional_data()?;
    println!("{info:?}");
}
```

Run the example:

```bash
cargo run --example example --features serde,serde_json
```

## Platform Data Sources

### Linux

The crate reads and merges these sources:

- `Machine ID` from `hostnamectl status`
- `/var/lib/dbus/machine-id`
- `/etc/machine-id`
- `/sys/class/dmi/id/product_uuid`

Linux machine-id values can be changed by users or system processes, so they should not be treated as a strong security boundary.

### macOS

The crate reads stable hardware fields from `system_profiler SPHardwareDataType SPSecureElementDataType`:

- Model Number
- Serial Number
- Hardware UUID
- SEID

`additional_data()` also reads username, hostname, OS version, chip, memory size, CPU core count, and language settings. These fields are not used in the hash.

### Windows

The crate reads these PowerShell/WMI fields:

- `Win32_ComputerSystemProduct.UUID`
- `Win32_BIOS.SerialNumber`
- `Win32_BaseBoard.SerialNumber`
- `Win32_Processor.ProcessorId`

## Cleanup Scope

Compared with the source project [doroved/mid](https://github.com/doroved/mid), this repository has been simplified as follows:

- Removed iOS and Android source entry points.
- Removed iOS build scripts, Xcode example project, xcframework artifacts, and mobile documentation.
- Removed mobile-only dependencies and `staticlib` output configuration.
- Kept the Linux, macOS, and Windows Rust APIs.
- Rewrote the README so it matches the current desktop-only scope.

## License and Final Copyright

The source project [doroved/mid](https://github.com/doroved/mid) is licensed under `MIT OR Apache-2.0`. This project keeps the same dual-license model. See:

- [LICENSE-MIT](./LICENSE-MIT)
- [LICENSE-APACHE](./LICENSE-APACHE)
