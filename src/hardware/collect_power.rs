use std::process::Command;
use std::fs;
use crate::hardware::types::PowerSupplyInfo;

pub fn collect_power_supplies() -> Vec<PowerSupplyInfo> {
    let mut power_supplies = Vec::new();
    
    // Try multiple methods to detect power supplies
    
    // 1. Try dmidecode for power supply information (requires root)
    if let Some(mut psu_vec) = collect_power_supplies_dmidecode() {
        power_supplies.append(&mut psu_vec);
    }
    
    // 2. Try IPMI for power supply information (if available)
    if let Some(mut psu_vec) = collect_power_supplies_ipmi() {
        power_supplies.append(&mut psu_vec);
    }
    
    // 3. Try lshw for power supply information
    if let Some(mut psu_vec) = collect_power_supplies_lshw() {
        power_supplies.append(&mut psu_vec);
    }
    
    // 4. Check for UPS information via apcupsd or similar
    if let Some(mut ups_vec) = collect_ups_information() {
        power_supplies.append(&mut ups_vec);
    }
    
    // 5. Try reading from sysfs power supply class
    if let Some(mut sysfs_vec) = collect_power_supplies_sysfs() {
        power_supplies.append(&mut sysfs_vec);
    }
    
    power_supplies
}

/// Collect power supply information using dmidecode
fn collect_power_supplies_dmidecode() -> Option<Vec<PowerSupplyInfo>> {
    let output = Command::new("dmidecode")
        .args(&["-t", "power", "-t", "powersupply"])
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let text = String::from_utf8_lossy(&output.stdout);
    let mut power_supplies = Vec::new();
    let mut current_psu = PowerSupplyInfo {
        name: None,
        manufacturer: None,
        model: None,
        serial_number: None,
        part_number: None,
        max_power_watts: None,
        efficiency_rating: None,
        status: None,
        input_voltage: None,
        input_current: None,
        output_voltage: None,
        output_current: None,
        temperature_c: None,
        fan_speed_rpm: None,
    };
    let mut in_power_supply = false;
    
    for line in text.lines() {
        let line = line.trim();
        
        if line.contains("Power Supply") && line.contains("Handle") {
            // Save previous PSU if we were processing one
            if in_power_supply {
                power_supplies.push(current_psu);
                current_psu = PowerSupplyInfo {
                    name: None,
                    manufacturer: None,
                    model: None,
                    serial_number: None,
                    part_number: None,
                    max_power_watts: None,
                    efficiency_rating: None,
                    status: None,
                    input_voltage: None,
                    input_current: None,
                    output_voltage: None,
                    output_current: None,
                    temperature_c: None,
                    fan_speed_rpm: None,
                };
            }
            in_power_supply = true;
        } else if in_power_supply && line.contains(":") {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                
                if !value.is_empty() && value != "Not Specified" && value != "To Be Filled By O.E.M." {
                    match key {
                        "Manufacturer" => current_psu.manufacturer = Some(value.to_string()),
                        "Model" => current_psu.model = Some(value.to_string()),
                        "Serial Number" => current_psu.serial_number = Some(value.to_string()),
                        "Part Number" => current_psu.part_number = Some(value.to_string()),
                        "Name" => current_psu.name = Some(value.to_string()),
                        "Max Power Capacity" => {
                            if let Some(watts_str) = value.split_whitespace().next() {
                                if let Ok(watts) = watts_str.parse::<u32>() {
                                    current_psu.max_power_watts = Some(watts);
                                }
                            }
                        },
                        "Status" => current_psu.status = Some(value.to_string()),
                        _ => {}
                    }
                }
            }
        }
    }
    
    // Don't forget the last PSU
    if in_power_supply {
        power_supplies.push(current_psu);
    }
    
    if power_supplies.is_empty() {
        None
    } else {
        Some(power_supplies)
    }
}

/// Collect power supply information using IPMI
fn collect_power_supplies_ipmi() -> Option<Vec<PowerSupplyInfo>> {
    let output = Command::new("ipmitool")
        .args(&["sdr", "list", "full"])
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let text = String::from_utf8_lossy(&output.stdout);
    let mut power_supplies = Vec::new();
    
    for line in text.lines() {
        if line.to_lowercase().contains("power") && 
           (line.to_lowercase().contains("supply") || line.to_lowercase().contains("psu")) {
            
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                let name = parts[0].trim();
                let status = parts[2].trim();
                
                let mut psu = PowerSupplyInfo {
                    name: Some(name.to_string()),
                    manufacturer: None,
                    model: None,
                    serial_number: None,
                    part_number: None,
                    max_power_watts: None,
                    efficiency_rating: None,
                    status: Some(status.to_string()),
                    input_voltage: None,
                    input_current: None,
                    output_voltage: None,
                    output_current: None,
                    temperature_c: None,
                    fan_speed_rpm: None,
                };
                
                // Try to get more detailed info for this power supply
                if let Some(detailed_info) = get_ipmi_psu_details(name) {
                    if let Some(temp) = detailed_info.temperature_c {
                        psu.temperature_c = Some(temp);
                    }
                    if let Some(voltage) = detailed_info.output_voltage {
                        psu.output_voltage = Some(voltage);
                    }
                }
                
                power_supplies.push(psu);
            }
        }
    }
    
    if power_supplies.is_empty() {
        None
    } else {
        Some(power_supplies)
    }
}

/// Get detailed IPMI information for a specific PSU
fn get_ipmi_psu_details(psu_name: &str) -> Option<PowerSupplyInfo> {
    // Try to get sensor readings for this PSU
    let output = Command::new("ipmitool")
        .args(&["sdr", "get", psu_name])
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let text = String::from_utf8_lossy(&output.stdout);
    let mut temperature_c = None;
    let mut voltage = None;
    
    for line in text.lines() {
        if line.contains("Sensor Reading") {
            if let Some(value_str) = line.split(':').nth(1) {
                let value_str = value_str.trim();
                
                // Parse temperature
                if value_str.contains("degrees C") {
                    if let Some(temp_str) = value_str.split_whitespace().next() {
                        if let Ok(temp) = temp_str.parse::<i32>() {
                            temperature_c = Some(temp);
                        }
                    }
                }
                
                // Parse voltage
                if value_str.contains("Volts") {
                    if let Some(volt_str) = value_str.split_whitespace().next() {
                        if let Ok(volt) = volt_str.parse::<f32>() {
                            voltage = Some(volt);
                        }
                    }
                }
            }
        }
    }
    
    Some(PowerSupplyInfo {
        name: Some(psu_name.to_string()),
        manufacturer: None,
        model: None,
        serial_number: None,
        part_number: None,
        max_power_watts: None,
        efficiency_rating: None,
        status: None,
        input_voltage: None,
        input_current: None,
        output_voltage: voltage,
        output_current: None,
        temperature_c,
        fan_speed_rpm: None,
    })
}

/// Collect power supply information using lshw
fn collect_power_supplies_lshw() -> Option<Vec<PowerSupplyInfo>> {
    let output = Command::new("lshw")
        .args(&["-class", "power"])
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let text = String::from_utf8_lossy(&output.stdout);
    
    // Basic parsing - lshw doesn't usually show much PSU info
    if text.contains("power") {
        let psu = PowerSupplyInfo {
            name: Some("System Power Supply".to_string()),
            manufacturer: None,
            model: None,
            serial_number: None,
            part_number: None,
            max_power_watts: None,
            efficiency_rating: None,
            status: Some("Present".to_string()),
            input_voltage: None,
            input_current: None,
            output_voltage: None,
            output_current: None,
            temperature_c: None,
            fan_speed_rpm: None,
        };
        Some(vec![psu])
    } else {
        None
    }
}

/// Collect UPS information
fn collect_ups_information() -> Option<Vec<PowerSupplyInfo>> {
    // Try apcupsd
    if let Some(ups) = collect_apcupsd_info() {
        return Some(vec![ups]);
    }
    
    // Try nut (Network UPS Tools)
    if let Some(ups) = collect_nut_info() {
        return Some(vec![ups]);
    }
    
    None
}

/// Collect APC UPS information via apcupsd
fn collect_apcupsd_info() -> Option<PowerSupplyInfo> {
    let output = Command::new("apcaccess")
        .args(&["status"])
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let text = String::from_utf8_lossy(&output.stdout);
    let mut ups = PowerSupplyInfo {
        name: Some("UPS".to_string()),
        manufacturer: Some("APC".to_string()),
        model: None,
        serial_number: None,
        part_number: None,
        max_power_watts: None,
        efficiency_rating: None,
        status: None,
        input_voltage: None,
        input_current: None,
        output_voltage: None,
        output_current: None,
        temperature_c: None,
        fan_speed_rpm: None,
    };
    
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            
            match key {
                "MODEL" => ups.model = Some(value.to_string()),
                "SERIALNO" => ups.serial_number = Some(value.to_string()),
                "STATUS" => ups.status = Some(value.to_string()),
                "LINEV" => {
                    if let Some(voltage_str) = value.split_whitespace().next() {
                        if let Ok(voltage) = voltage_str.parse::<f32>() {
                            ups.input_voltage = Some(voltage);
                        }
                    }
                },
                "OUTPUTV" => {
                    if let Some(voltage_str) = value.split_whitespace().next() {
                        if let Ok(voltage) = voltage_str.parse::<f32>() {
                            ups.output_voltage = Some(voltage);
                        }
                    }
                },
                "ITEMP" => {
                    if let Some(temp_str) = value.split_whitespace().next() {
                        if let Ok(temp) = temp_str.parse::<i32>() {
                            ups.temperature_c = Some(temp);
                        }
                    }
                },
                _ => {}
            }
        }
    }
    
    Some(ups)
}

/// Collect NUT (Network UPS Tools) information
fn collect_nut_info() -> Option<PowerSupplyInfo> {
    let output = Command::new("upsc")
        .args(&["ups"])
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let text = String::from_utf8_lossy(&output.stdout);
    let mut ups = PowerSupplyInfo {
        name: Some("UPS".to_string()),
        manufacturer: None,
        model: None,
        serial_number: None,
        part_number: None,
        max_power_watts: None,
        efficiency_rating: None,
        status: None,
        input_voltage: None,
        input_current: None,
        output_voltage: None,
        output_current: None,
        temperature_c: None,
        fan_speed_rpm: None,
    };
    
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            
            match key {
                "device.mfr" => ups.manufacturer = Some(value.to_string()),
                "device.model" => ups.model = Some(value.to_string()),
                "device.serial" => ups.serial_number = Some(value.to_string()),
                "ups.status" => ups.status = Some(value.to_string()),
                "input.voltage" => {
                    if let Ok(voltage) = value.parse::<f32>() {
                        ups.input_voltage = Some(voltage);
                    }
                },
                "output.voltage" => {
                    if let Ok(voltage) = value.parse::<f32>() {
                        ups.output_voltage = Some(voltage);
                    }
                },
                "ups.temperature" => {
                    if let Ok(temp) = value.parse::<i32>() {
                        ups.temperature_c = Some(temp);
                    }
                },
                _ => {}
            }
        }
    }
    
    Some(ups)
}

/// Collect power supply information from sysfs
fn collect_power_supplies_sysfs() -> Option<Vec<PowerSupplyInfo>> {
    let power_supply_path = "/sys/class/power_supply";
    
    let entries = fs::read_dir(power_supply_path).ok()?;
    let mut power_supplies = Vec::new();
    
    for entry in entries.flatten() {
        let psu_name = entry.file_name();
        let psu_name_str = psu_name.to_str()?;
        
        // Skip batteries and other non-PSU devices
        if psu_name_str.starts_with("BAT") || psu_name_str.starts_with("ADP") {
            continue;
        }
        
        let psu_path = entry.path();
        let mut psu = PowerSupplyInfo {
            name: Some(psu_name_str.to_string()),
            manufacturer: None,
            model: None,
            serial_number: None,
            part_number: None,
            max_power_watts: None,
            efficiency_rating: None,
            status: None,
            input_voltage: None,
            input_current: None,
            output_voltage: None,
            output_current: None,
            temperature_c: None,
            fan_speed_rpm: None,
        };
        
        // Read various power supply attributes
        if let Ok(status) = fs::read_to_string(psu_path.join("status")) {
            psu.status = Some(status.trim().to_string());
        }
        
        if let Ok(manufacturer) = fs::read_to_string(psu_path.join("manufacturer")) {
            psu.manufacturer = Some(manufacturer.trim().to_string());
        }
        
        if let Ok(model) = fs::read_to_string(psu_path.join("model_name")) {
            psu.model = Some(model.trim().to_string());
        }
        
        if let Ok(serial) = fs::read_to_string(psu_path.join("serial_number")) {
            psu.serial_number = Some(serial.trim().to_string());
        }
        
        power_supplies.push(psu);
    }
    
    if power_supplies.is_empty() {
        None
    } else {
        Some(power_supplies)
    }
}