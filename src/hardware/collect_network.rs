use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use pciid_parser::Database;
use serde_json::Value;

use crate::hardware::types::{IpAddress, NetInterface, NetworkInfo, RouteInfo};

/// Entry point: collect full network info (interfaces + routes).
pub fn collect_network_info() -> NetworkInfo {
    let iface_addrs = collect_ip_addrs();
    let routes = collect_routes();

    let mut interfaces = Vec::new();
    let sys_class_net = Path::new("/sys/class/net");

    let entries = match fs::read_dir(sys_class_net) {
        Ok(e) => e,
        Err(_) => {
            return NetworkInfo {
                interfaces,
                routes,
            }
        }
    };

    for entry in entries.flatten() {
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Skip virtual interfaces - only collect physical NICs
        if is_virtual_interface(&name, &entry.path()) {
            continue;
        }

        let iface_sys_path = entry.path();

        let mac_address = read_to_string_trim(iface_sys_path.join("address"));
        let mtu = read_to_u32(iface_sys_path.join("mtu"));

        // Speed can be in sysfs or via ethtool as fallback
        let speed_mbps = read_to_u32(iface_sys_path.join("speed"))
            .or_else(|| ethtool_speed(&name));

        let driver = read_driver(&iface_sys_path.join("device"));

        // PCI address from device path
        let pci_address = read_pci_address(&iface_sys_path.join("device"));

        // Firmware version from ethtool -i
        let firmware_version = ethtool_firmware(&name);

        // Vendor/Device information for PCI devices
        let (vendor_name, device_name) = read_vendor_device_info(&iface_sys_path);

        // Addresses from ip -j addr
        let addresses = iface_addrs.get(&name).cloned().unwrap_or_default();

        // Bond/team configuration
        let (is_primary, bond_group, bond_master) = detect_bond_info(&name, &iface_sys_path);

        interfaces.push(NetInterface {
            name,
            mac_address,
            mtu,
            speed_mbps,
            driver,
            firmware_version,
            vendor_name,
            device_name,
            pci_address,
            addresses,
            is_primary,
            bond_group,
            bond_master,
        });
    }

    NetworkInfo {
        interfaces,
        routes,
    }
}

//
// Interfaces: /sys helpers
//

fn read_to_string_trim<P: AsRef<Path>>(path: P) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_to_u32<P: AsRef<Path>>(path: P) -> Option<u32> {
    let s = read_to_string_trim(path)?;
    s.parse::<u32>().ok()
}

/// Get driver name via /sys/class/net/<iface>/device/driver -> symlink basename.
fn read_driver(device_path: &Path) -> Option<String> {
    let driver_link = device_path.join("driver");
    let link = fs::read_link(driver_link).ok()?;
    link.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Read PCI address from /sys/class/net/<iface>/device symlink
fn read_pci_address(device_path: &Path) -> Option<String> {
    // Follow the device symlink to get the real path
    let link_target = fs::read_link(device_path).ok()?;
    let real_path = if link_target.is_absolute() {
        link_target
    } else {
        device_path.parent()?.join(link_target)
    };

    // Look through the path components for a PCI address
    for component in real_path.components() {
        if let Some(name) = component.as_os_str().to_str() {
            if is_pci_address(name) {
                return Some(name.to_string());
            }
        }
    }

    None
}

fn is_pci_address(s: &str) -> bool {
    // PCI address format: 0000:3b:00.0 (domain:bus:device.function)
    s.len() >= 12 && s.matches(':').count() == 2 && s.contains('.')
}

/// Check if a network interface is virtual (not a physical NIC)
fn is_virtual_interface(name: &str, iface_sys_path: &Path) -> bool {
    // Common virtual interface name patterns
    if name.starts_with("lo")           // loopback
        || name.starts_with("veth")     // virtual ethernet (Docker, etc.)
        || name.starts_with("docker")   // Docker bridge
        || name.starts_with("br-")      // bridge interfaces
        || name.starts_with("virbr")    // libvirt bridge
        || name.starts_with("cni")      // Container Network Interface
        || name.starts_with("flannel")  // Kubernetes flannel
        || name.starts_with("kube")     // Kubernetes interfaces
        || name.starts_with("tun")      // tunnel interfaces
        || name.starts_with("tap")      // tap interfaces
        || name.starts_with("vmnet")    // VMware interfaces
        || name.contains("vlan")        // VLAN interfaces
    {
        return true;
    }

    // Bond and team interfaces are now included - we want to track them
    // Check if interface has a PCI device (physical interfaces will have one)
    // Bond/team interfaces won't have PCI but should still be tracked
    if name.starts_with("bond") || name.starts_with("team") {
        return false; // Include bond/team interfaces
    }

    let device_path = iface_sys_path.join("device");
    !device_path.exists() // No device path = virtual
}

/// Read vendor/device information for a network interface
fn read_vendor_device_info(iface_sys_path: &Path) -> (Option<String>, Option<String>) {
    let device_path = iface_sys_path.join("device");
    
    // Skip virtual devices (they don't have PCI device info)
    if !device_path.exists() {
        return (None, None);
    }

    // Read vendor/device IDs from sysfs and look up in PCI database
    let vendor_id = read_to_string_trim(device_path.join("vendor"));
    let device_id = read_to_string_trim(device_path.join("device"));

    if let (Some(vendor), Some(device)) = (vendor_id, device_id) {
        if let Some((vendor_name, device_name)) = lookup_pci_ids(&vendor, &device) {
            return (Some(vendor_name), Some(device_name));
        }
    }

    (None, None)
}

/// Look up vendor and device names using PCI database
fn lookup_pci_ids(vendor_hex: &str, device_hex: &str) -> Option<(String, String)> {
    // Parse hex IDs
    let vendor_id = u16::from_str_radix(
        vendor_hex.strip_prefix("0x").unwrap_or(vendor_hex), 
        16
    ).ok()?;
    let device_id = u16::from_str_radix(
        device_hex.strip_prefix("0x").unwrap_or(device_hex), 
        16
    ).ok()?;

    // Load PCI database from system default paths
    let db = Database::read().ok()?;
    
    // Get vendor - this should always work if the vendor exists
    let vendor = db.vendors.get(&vendor_id)?;
    let vendor_name = vendor.name.clone();
    
    // Get device - fallback to "Unknown Device" if device not found
    let device_name = vendor.devices.get(&device_id)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| format!("Unknown Device [{}]", device_hex));
    
    Some((vendor_name, device_name))
}

//
// ethtool fallbacks
//

fn ethtool_speed(iface: &str) -> Option<u32> {
    let output = Command::new("ethtool")
        .arg(iface)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        // Example: "Speed: 25000Mb/s"
        if let Some(rest) = line.trim().strip_prefix("Speed:") {
            let part = rest.trim().split_whitespace().next().unwrap_or("");
            // strip "Mb/s"
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(speed) = digits.parse::<u32>() {
                return Some(speed);
            }
        }
    }
    None
}

fn ethtool_firmware(iface: &str) -> Option<String> {
    let output = Command::new("ethtool")
        .args(["-i", iface])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut firmware_version = None;
    let mut driver_version = None;
    
    for line in text.lines() {
        let line = line.trim();
        // Look for firmware version first (preferred)
        if let Some(rest) = line.strip_prefix("firmware-version:") {
            let version = rest.trim().to_string();
            if !version.is_empty() && version != "N/A" && version != "n/a" {
                firmware_version = Some(version);
            }
        } else if let Some(rest) = line.strip_prefix("version:") {
            let version = rest.trim().to_string();
            if !version.is_empty() && version != "N/A" && version != "n/a" {
                driver_version = Some(version);
            }
        }
    }
    
    // Prefer firmware-version over driver version
    firmware_version.or(driver_version)
}

//
// IP addresses via `ip -j addr`
//

fn collect_ip_addrs() -> HashMap<String, Vec<IpAddress>> {
    let mut map: HashMap<String, Vec<IpAddress>> = HashMap::new();

    let output = Command::new("ip")
        .args(["-j", "addr"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return map,
    };

    let json: Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return map,
    };

    let arr = match json.as_array() {
        Some(a) => a,
        None => return map,
    };

    for iface in arr {
        let ifname = iface.get("ifname").and_then(|v| v.as_str());
        let ifname = match ifname {
            Some(n) => n.to_string(),
            None => continue,
        };

        let mut addrs = Vec::new();

        if let Some(addr_info) = iface.get("addr_info").and_then(|v| v.as_array()) {
            for addr in addr_info {
                let family = addr.get("family").and_then(|v| v.as_str()).unwrap_or("");
                let local = addr.get("local").and_then(|v| v.as_str()).unwrap_or("");
                let prefix = addr
                    .get("prefixlen")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u8;

                if local.is_empty() {
                    continue;
                }

                addrs.push(IpAddress {
                    family: family.to_string(),
                    address: local.to_string(),
                    prefix,
                });
            }
        }

        map.insert(ifname, addrs);
    }

    map
}

//
// Routes via `ip -j route`
//

fn collect_routes() -> Vec<RouteInfo> {
    let mut routes = Vec::new();

    let output = Command::new("ip")
        .args(["-j", "route"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return routes,
    };

    let json: Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return routes,
    };

    let arr = match json.as_array() {
        Some(a) => a,
        None => return routes,
    };

    for r in arr {
        let dst = r
            .get("dst")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();
        let gateway = r
            .get("gateway")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let iface = r
            .get("dev")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        routes.push(RouteInfo { dst, gateway, iface });
    }

    routes
}

/// Detect bond/team configuration for a network interface
fn detect_bond_info(iface: &str, iface_sys_path: &Path) -> (bool, Option<String>, Option<String>) {
    // Default values
    let mut is_primary = false;
    let mut bond_group = None;
    let mut bond_master = None;

    // Check if this is a bond/team master interface
    if iface.starts_with("bond") || iface.starts_with("team") {
        is_primary = true;
        bond_group = Some(iface.to_string());
        return (is_primary, bond_group, bond_master);
    }

    // Check if this interface is enslaved to a bond
    let master_path = iface_sys_path.join("master");
    if let Ok(master_link) = fs::read_link(&master_path) {
        if let Some(master_name) = master_link.file_name().and_then(|n| n.to_str()) {
            bond_group = Some(master_name.to_string());
            bond_master = Some(master_name.to_string());
            
            // If this interface is part of a bond, it's part of the primary configuration
            if master_name.starts_with("bond") || master_name.starts_with("team") {
                is_primary = true;
            }
        }
    }

    // For simple heuristic: if interface has an IP address, it might be primary
    // This is a fallback for interfaces that aren't in bonds but are the main interface
    if !is_primary {
        // Check if interface has any IP addresses configured
        let output = Command::new("ip")
            .args(["-j", "addr", "show", iface])
            .output();

        if let Ok(output) = output {
            if let Ok(json) = serde_json::from_slice::<Value>(&output.stdout) {
                if let Some(arr) = json.as_array() {
                    for iface_data in arr {
                        if let Some(addr_info) = iface_data.get("addr_info").and_then(|v| v.as_array()) {
                            for addr in addr_info {
                                let family = addr.get("family").and_then(|v| v.as_str()).unwrap_or("");
                                let scope = addr.get("scope").and_then(|v| v.as_str()).unwrap_or("");
                                
                                // If it has a global IPv4 address, consider it primary
                                if family == "inet" && scope == "global" {
                                    is_primary = true;
                                    break;
                                }
                            }
                            if is_primary { break; }
                        }
                    }
                }
            }
        }
    }

    (is_primary, bond_group, bond_master)
}
