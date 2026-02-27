use std::fs;
use std::process::Command;
use smbioslib::*;
use crate::hardware::types::{NodeInfo, BiosInfo, BmcInfo, MotherboardInfo};

pub fn collect_node_info() -> NodeInfo {
    let hostname = get_hostname();
    let architecture = std::env::consts::ARCH.to_string();
    
    // Collect all DMI information using smbios-lib
    let (product_name, manufacturer, serial_number, chassis_manufacturer, chassis_serial_number, motherboard, bios) = 
        collect_dmi_info();
    
    let bmc = Some(collect_bmc_from_dmi());

    NodeInfo {
        hostname,
        architecture,
        product_name,
        manufacturer,
        serial_number,
        chassis_manufacturer,
        chassis_serial_number,
        motherboard,
        bios,
        bmc,
    }
}

fn get_hostname() -> String {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string()
}

fn collect_dmi_info() -> (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<MotherboardInfo>, Option<BiosInfo>) {
    // Try to load SMBIOS data from the system
    let smbios_data = match SMBiosData::try_load_from_file("/sys/firmware/dmi/tables/DMI", None) {
        Ok(data) => data,
        Err(_) => {
            // If that fails, try reading the raw data and parsing it
            match fs::read("/sys/firmware/dmi/tables/DMI") {
                Ok(table_data) => {
                    SMBiosData::from_vec_and_version(table_data, None)
                },
                Err(_) => return (None, None, None, None, None, None, None),
            }
        }
    };

    let mut system_info = (None, None, None, None);
    let mut chassis_manufacturer = None;
    let mut chassis_serial_number = None;
    let mut motherboard_info = None;
    let mut bios_info = None;

    // Iterate through SMBIOS structures
    for structure in smbios_data.iter() {
        match structure.defined_struct() {
            DefinedStruct::SystemInformation(system_struct) => {
                // Extract system information
                let product_name = system_struct.product_name()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty() && s != "Not Specified")
                    .map(|s| s.to_string());
                
                let manufacturer = system_struct.manufacturer()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty() && s != "Not Specified")
                    .map(|s| s.to_string());
                
                let serial_number = system_struct.serial_number()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty() && s != "Not Specified" && s != "Not Available")
                    .map(|s| s.to_string());
                
                let sku_number = system_struct.sku_number()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty() && s != "Not Specified")
                    .map(|s| s.to_string());
                
                system_info = (product_name, manufacturer, serial_number, sku_number);
            },
            
            DefinedStruct::SystemChassisInformation(chassis_struct) => {
                // Extract chassis manufacturer
                let manufacturer = chassis_struct.manufacturer()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty() && s != "Not Specified" && s != "To Be Filled By O.E.M." && s != "Default string")
                    .map(|s| s.to_string());
                chassis_manufacturer = manufacturer;
                
                // Extract chassis serial number
                let serial_number = chassis_struct.serial_number()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty() && s != "Not Specified" && s != "To Be Filled By O.E.M." && s != "Default string")
                    .map(|s| s.to_string());
                chassis_serial_number = serial_number;
            },
            
            DefinedStruct::BaseBoardInformation(baseboard_struct) => {
                // Extract motherboard information
                let manufacturer = baseboard_struct.manufacturer()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty() && s != "Not Specified" && s != "To Be Filled By O.E.M.")
                    .map(|s| s.to_string());
                
                let product_name = baseboard_struct.product()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty() && s != "Not Specified" && s != "To Be Filled By O.E.M.")
                    .map(|s| s.to_string());
                
                let version = baseboard_struct.version()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty() && s != "Not Specified" && s != "To Be Filled By O.E.M.")
                    .map(|s| s.to_string());
                
                let serial_number = baseboard_struct.serial_number()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty() && s != "Not Specified" && s != "To Be Filled By O.E.M.")
                    .map(|s| s.to_string());
                
                if manufacturer.is_some() || product_name.is_some() || version.is_some() || serial_number.is_some() {
                    motherboard_info = Some(MotherboardInfo {
                        manufacturer,
                        product_name,
                        version,
                        serial_number,
                    });
                }
            },
            
            DefinedStruct::Information(bios_struct) => {
                // Extract BIOS information
                let vendor = bios_struct.vendor()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                
                let version = bios_struct.version()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                
                let release_date = bios_struct.release_date()
                    .to_utf8_lossy()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                
                if vendor.is_some() || version.is_some() || release_date.is_some() {
                    bios_info = Some(BiosInfo {
                        vendor,
                        version,
                        release_date,
                    });
                }
            },
            
            _ => continue,
        }
    }

    // Return system info with chassis manufacturer and serial number
    (system_info.0, system_info.1, system_info.2, chassis_manufacturer, chassis_serial_number, motherboard_info, bios_info)
}

fn collect_bmc_from_dmi() -> BmcInfo {
    // Try IPMI device file detection first (most reliable)
    if let Some(bmc) = detect_ipmi_device() {
        return bmc;
    }

    // Try SMBIOS BMC detection
    if let Some(bmc) = detect_smbios_bmc() {
        return bmc;
    }

    // Try network-based management interface detection
    if let Some(bmc) = detect_network_management() {
        return bmc;
    }

    // Try IPMI tools if available
    if let Some(ipmi_bmc) = collect_ipmi_bmc() {
        return ipmi_bmc;
    }

    // Try Redfish detection
    if let Some(redfish_bmc) = collect_redfish_bmc() {
        return redfish_bmc;
    }

    // Return empty BMC info if no management controller detected
    BmcInfo {
        ip_address: None,
        mac_address: None,
        firmware_version: None,
        release_date: None,
    }
}

fn detect_network_management() -> Option<BmcInfo> {
    // Check for common management network interfaces or services
    // Look for interfaces that might be management-related
    let mgmt_patterns = ["bmc", "ipmi", "ilo", "idrac", "rac"];
    
    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            if let Some(interface_name) = entry.file_name().to_str() {
                let name_lower = interface_name.to_lowercase();
                for pattern in &mgmt_patterns {
                    if name_lower.contains(pattern) {
                        return Some(BmcInfo {
                            ip_address: None,
                            mac_address: None,
                            firmware_version: None,
                            release_date: None,
                        });
                    }
                }
            }
        }
    }
    
    // Check for common management ports (this is speculative)
    let mgmt_ports = [623, 443, 80]; // IPMI, HTTPS, HTTP
    for port in &mgmt_ports {
        if let Ok(output) = Command::new("netstat")
            .args(&["-ln"])
            .output()
        {
            if output.status.success() {
                let netstat_output = String::from_utf8_lossy(&output.stdout);
                if netstat_output.contains(&format!(":{}", port)) && *port == 623 {
                    return Some(BmcInfo {
                        ip_address: None,
                        mac_address: None,
                        firmware_version: None,
                        release_date: None,
                    });
                }
            }
        }
    }
    
    None
}

fn detect_ipmi_device() -> Option<BmcInfo> {
    // Check for common IPMI device files
    let ipmi_devices = ["/dev/ipmi0", "/dev/ipmidev/0", "/dev/ipmi/0"];
    
    for device in &ipmi_devices {
        if std::path::Path::new(device).exists() {
            return Some(BmcInfo {
                ip_address: None, // Would need ipmitool to get network info
                mac_address: None,
                firmware_version: None,
                release_date: None,
            });
        }
    }
    
    None
}

fn detect_smbios_bmc() -> Option<BmcInfo> {
    // For now, we'll disable SMBIOS BMC detection due to API issues
    // This could be re-implemented with proper SMBIOS structure handling
    None
}

fn collect_ipmi_bmc() -> Option<BmcInfo> {
    // Check if ipmitool exists first
    if Command::new("which").arg("ipmitool").output().is_err() {
        return None;
    }
    
    // Try ipmitool mc info
    if let Ok(output) = Command::new("ipmitool")
        .args(&["mc", "info"])
        .output()
    {
        if output.status.success() {
            let mut firmware_version = None;
            let mut release_date = None;
            
            let ipmi_output = String::from_utf8_lossy(&output.stdout);
            for line in ipmi_output.lines() {
                if line.contains("Firmware Revision") {
                    if let Some(version) = line.split(':').nth(1) {
                        firmware_version = Some(version.trim().to_string());
                    }
                }
                // Look for firmware build date or similar
                if line.contains("Build Time") || line.contains("Build Date") || line.contains("Firmware Build") {
                    if let Some(date) = line.split(':').nth(1) {
                        release_date = Some(date.trim().to_string());
                    }
                }
            }

            // Try to get network info
            let (ip_address, mac_address) = get_ipmi_network_info();

            return Some(BmcInfo {
                ip_address,
                mac_address,
                firmware_version,
                release_date,
            });
        }
    }

    None
}

fn get_ipmi_network_info() -> (Option<String>, Option<String>) {
    let mut ip_address = None;
    let mut mac_address = None;

    // Try to get LAN configuration from ipmitool
    if let Ok(output) = Command::new("ipmitool")
        .args(&["lan", "print", "1"])
        .output()
    {
        if output.status.success() {
            let lan_output = String::from_utf8_lossy(&output.stdout);
            
            for line in lan_output.lines() {
                if line.contains("IP Address") && !line.contains("Source") {
                    if let Some(ip) = line.split(':').nth(1) {
                        let ip_str = ip.trim();
                        if ip_str != "0.0.0.0" && !ip_str.is_empty() {
                            ip_address = Some(ip_str.to_string());
                        }
                    }
                } else if line.contains("MAC Address") {
                    if let Some(mac) = line.split(':').nth(1) {
                        // MAC address format in ipmitool output: "aa:bb:cc:dd:ee:ff"
                        // We need to handle the rest of the line after the first colon
                        let mac_str = line.split_once(':')
                            .and_then(|(_, rest)| rest.split_once(':'))
                            .map(|(first_octet, rest)| format!("{}:{}", first_octet.trim(), rest))
                            .unwrap_or_else(|| mac.trim().to_string());
                        
                        if !mac_str.is_empty() && mac_str != "00:00:00:00:00:00" {
                            mac_address = Some(mac_str);
                        }
                    }
                }
            }
        }
    }

    (ip_address, mac_address)
}

fn collect_redfish_bmc() -> Option<BmcInfo> {
    // Try to detect Redfish BMC
    // This is a basic implementation - in practice you'd need to:
    // 1. Check for known Redfish endpoints
    // 2. Try standard Redfish discovery methods
    // 3. Check vendor-specific indicators

    // Look for common Redfish indicators
    let redfish_indicators = [
        "/redfish/v1/",
        "/redfish/v1/Systems",
        "/redfish/v1/Managers",
    ];

    // Try curl to localhost with common Redfish paths
    for indicator in &redfish_indicators {
        let url = format!("https://localhost{}", indicator);
        
        if let Ok(output) = Command::new("curl")
            .args(&["-k", "-s", "--connect-timeout", "2", &url])
            .output()
        {
            if output.status.success() {
                let response = String::from_utf8_lossy(&output.stdout);
                if response.contains("@odata") || response.contains("redfish") {
                    return Some(BmcInfo {
                        ip_address: Some("localhost".to_string()),
                        mac_address: None, // Would need additional API calls
                        firmware_version: None, // Would need additional API calls
                        release_date: None, // Would need additional API calls
                    });
                }
            }
        }
    }

    None
}