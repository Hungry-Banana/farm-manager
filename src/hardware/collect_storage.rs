use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::hardware::types::{DiskInfo, SmartInfo};

/// Entry point: collect all disks on this machine.
pub fn collect_disks() -> Vec<DiskInfo> {
    let mut disks = Vec::new();
    let sys_block = Path::new("/sys/block");

    let entries = match fs::read_dir(sys_block) {
        Ok(e) => e,
        Err(_) => return disks,
    };

    for entry in entries.flatten() {
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Skip virtual / non-physical devices (tweak as you like)
        if name.starts_with("loop")
            || name.starts_with("ram")
            || name.starts_with("dm-")
            || name.starts_with("zram")
        {
            continue;
        }

        let sys_path = entry.path();
        let dev_path = format!("/dev/{}", name);

        // Skip if the device file doesn't actually exist
        if !Path::new(&dev_path).exists() {
            continue;
        }

        let disk = collect_single_disk(&name, &sys_path, &dev_path);
        disks.push(disk);
    }

    disks
}

/// Collect detailed info for a single disk.
fn collect_single_disk(name: &str, sys_path: &Path, dev_path: &str) -> DiskInfo {
    let device_path = sys_path.join("device");

    // Model (SATA/SAS and sometimes NVMe)
    let model = read_to_string_trim(device_path.join("model"));

    // Serial:
    //  - For SCSI-like devices: /sys/block/<dev>/device/serial
    //  - For NVMe: /sys/class/nvme/<ctrl>/serial
    let mut serial = read_to_string_trim(device_path.join("serial"));

    // Capacity in bytes
    let size_sectors = read_to_u64(sys_path.join("size"));
    let size_bytes = size_sectors.map(|s| s * 512);

    // Rotational: 1 = HDD, 0 = SSD/NVMe
    let rotational = read_to_u64(sys_path.join("queue/rotational")).map(|v| v == 1);

    // Bus type / NVMe extras / PCI address
    let mut bus_type: Option<String> = None;
    let mut firmware_version: Option<String> = None;

    if name.starts_with("nvme") {
        // NVMe namespace: "nvme0n1" → controller "nvme0"
        let controller = name.split('n').next().unwrap_or(name);
        bus_type = Some("nvme".to_string());

        let nvme_ctrl_path = PathBuf::from("/sys/class/nvme").join(controller);

        // Firmware / serial from controller sysfs
        firmware_version = read_to_string_trim(nvme_ctrl_path.join("firmware_rev"));

        if serial.is_none() {
            serial = read_to_string_trim(nvme_ctrl_path.join("serial"));
        }
    } else {
        // SCSI-like (SATA/SAS/USB/etc.)
        bus_type = detect_bus_type(&sys_path.join("device"));
    }

    // Try to get firmware version from hdparm/smartctl if not found in sysfs
    if firmware_version.is_none() {
        firmware_version = get_firmware_version(dev_path, bus_type.as_deref());
    }

    // Try to get serial number from hdparm/smartctl if not found in sysfs
    if serial.is_none() {
        serial = get_serial_number(dev_path, bus_type.as_deref());
    }

    // SMART / health info (optional, best effort)
    let smart = collect_smart_info(dev_path, bus_type.as_deref());

    DiskInfo {
        name: name.to_string(),
        dev_path: dev_path.to_string(),
        model,
        serial,
        size_bytes,
        rotational,
        bus_type,
        firmware_version,
        smart,
    }
}

//
// Helper functions
//

fn read_to_string_trim<P: AsRef<Path>>(path: P) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn read_to_u64<P: AsRef<Path>>(path: P) -> Option<u64> {
    let s = read_to_string_trim(path)?;
    s.parse::<u64>().ok()
}

/// Get firmware version using hdparm -I or smartctl
fn get_firmware_version(dev_path: &str, bus_type: Option<&str>) -> Option<String> {
    // Try smartctl first (works for most devices including NVMe)
    if let Some(version) = get_firmware_from_smartctl(dev_path, bus_type) {
        return Some(version);
    }

    // Try hdparm for SATA drives
    if let Some("scsi") = bus_type {
        if let Some(version) = get_firmware_from_hdparm(dev_path) {
            return Some(version);
        }
    }

    None
}

/// Get firmware version from smartctl
fn get_firmware_from_smartctl(dev_path: &str, bus_type: Option<&str>) -> Option<String> {
    let mut args = vec!["-i"];  // info flag
    
    if let Some("nvme") = bus_type {
        args.extend_from_slice(&["-d", "nvme"]);
    }
    args.push(dev_path);

    let output = Command::new("smartctl")
        .args(&args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        // Look for firmware version lines in smartctl output
        if line.starts_with("Firmware Version:") {
            if let Some(version) = line.split(':').nth(1) {
                let version = version.trim().to_string();
                if !version.is_empty() {
                    return Some(version);
                }
            }
        }
        // Alternative patterns
        if line.starts_with("Firmware Revision:") {
            if let Some(version) = line.split(':').nth(1) {
                let version = version.trim().to_string();
                if !version.is_empty() {
                    return Some(version);
                }
            }
        }
    }

    None
}

/// Get firmware version from hdparm -I (for SATA drives)
fn get_firmware_from_hdparm(dev_path: &str) -> Option<String> {
    let output = Command::new("hdparm")
        .args(["-I", dev_path])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        // Look for firmware revision in hdparm output
        if line.contains("Firmware Revision:") || line.contains("FW Revision:") {
            if let Some(version) = line.split(':').nth(1) {
                let version = version.trim().to_string();
                if !version.is_empty() {
                    return Some(version);
                }
            }
        }
    }

    None
}

/// Get serial number using hdparm -I or smartctl
fn get_serial_number(dev_path: &str, bus_type: Option<&str>) -> Option<String> {
    // Try smartctl first (works for most devices including NVMe)
    if let Some(serial) = get_serial_from_smartctl(dev_path, bus_type) {
        return Some(serial);
    }

    // Try hdparm for SATA drives
    if let Some("scsi") = bus_type {
        if let Some(serial) = get_serial_from_hdparm(dev_path) {
            return Some(serial);
        }
    }

    None
}

/// Get serial number from smartctl
fn get_serial_from_smartctl(dev_path: &str, bus_type: Option<&str>) -> Option<String> {
    let mut args = vec!["-i"];  // info flag
    
    if let Some("nvme") = bus_type {
        args.extend_from_slice(&["-d", "nvme"]);
    }
    args.push(dev_path);

    let output = Command::new("smartctl")
        .args(&args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        // Look for serial number lines in smartctl output
        if line.starts_with("Serial Number:") || line.starts_with("Serial number:") {
            if let Some(serial) = line.split(':').nth(1) {
                let serial = serial.trim().to_string();
                if !serial.is_empty() {
                    return Some(serial);
                }
            }
        }
    }

    None
}

/// Get serial number from hdparm -I (for SATA drives)
fn get_serial_from_hdparm(dev_path: &str) -> Option<String> {
    let output = Command::new("hdparm")
        .args(["-I", dev_path])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        // Look for serial number in hdparm output
        if line.contains("Serial Number:") {
            if let Some(serial) = line.split(':').nth(1) {
                let serial = serial.trim().to_string();
                if !serial.is_empty() {
                    return Some(serial);
                }
            }
        }
    }

    None
}

/// Try to detect bus type using sysfs / udev info.
fn detect_bus_type(device_path: &Path) -> Option<String> {
    // Check the "subsystem" symlink, e.g. .../scsi, .../nvme, .../virtio
    let subsystem_link = device_path.join("subsystem");
    if let Ok(link) = fs::read_link(&subsystem_link) {
        if let Some(name) = link.file_name().and_then(|n| n.to_str()) {
            return Some(name.to_string()); // "scsi", "nvme", "virtio", ...
        }
    }

    // Fallback: udev property ID_BUS
    if let Some(bus) = read_udev_property_from_sys_device(device_path, "ID_BUS") {
        return Some(bus);
    }

    None
}

/// Read udev properties for a device node, e.g. /dev/sda.
fn read_udev_property(dev_path: &str, key: &str) -> Option<String> {
    let output = Command::new("udevadm")
        .args(["info", "--query=property", "--name", dev_path])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            if let Some(val) = rest.strip_prefix('=') {
                return Some(val.trim().to_string());
            }
        }
    }
    None
}

/// Similar to `read_udev_property`, but starting from a sysfs device path.
fn read_udev_property_from_sys_device(device_path: &Path, key: &str) -> Option<String> {
    let dev_node = find_dev_node_for_sys_device(device_path)?;
    read_udev_property(&dev_node, key)
}

/// Try to guess the /dev/... node for a given sysfs device.
///
/// This is not perfect, but for typical disks it will find something like /dev/sda.
fn find_dev_node_for_sys_device(device_path: &Path) -> Option<String> {
    // We try `../..` up to /sys/block/<name>, then infer /dev/<name>.
    for ancestor in device_path.ancestors() {
        if ancestor.parent()?.file_name()? == "block" {
            if let Some(name) = ancestor.file_name().and_then(|n| n.to_str()) {
                return Some(format!("/dev/{}", name));
            }
        }
    }
    None
}

//
// SMART / health
//

fn collect_smart_info(dev_path: &str, bus_type: Option<&str>) -> Option<SmartInfo> {
    // Try smartctl first (works for SATA/SAS, and also NVMe with -d nvme)
    if let Some(smart) = smartctl_health(dev_path, bus_type) {
        return Some(smart);
    }

    // Optionally: you can also try nvme-cli if smartctl is not available.
    if let Some(smart) = nvme_cli_smart(dev_path, bus_type) {
        return Some(smart);
    }

    None
}

/// Use smartctl to get basic health info.
/// Requires smartmontools installed, might require root.
fn smartctl_health(dev_path: &str, bus_type: Option<&str>) -> Option<SmartInfo> {
    // For NVMe drives, smartctl needs "-d nvme"
    let mut args: Vec<&str> = vec!["-H"]; // basic health
    if let Some("nvme") = bus_type {
        args.extend_from_slice(&["-d", "nvme"]);
    }
    args.push(dev_path);

    let output = Command::new("smartctl").args(&args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let health = if text.contains("PASSED") {
        Some("PASSED".to_string())
    } else if text.contains("FAILED") {
        Some("FAILED".to_string())
    } else {
        None
    };

    // If you want more info (temp, hours, wear) you can parse `smartctl -a` instead.
    Some(SmartInfo {
        health,
    })
}

/// Use `nvme smart-log` as a fallback for NVMe drives.
/// Requires nvme-cli, likely root.
fn nvme_cli_smart(dev_path: &str, bus_type: Option<&str>) -> Option<SmartInfo> {
    if bus_type != Some("nvme") {
        return None;
    }

    let output = Command::new("nvme")
        .args(["smart-log", dev_path])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(SmartInfo {
        health: None, // nvme-cli doesn't give a simple PASSED/FAILED string
    })
}
