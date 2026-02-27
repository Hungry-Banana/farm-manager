use std::fs;
use smbioslib::*;
use crate::hardware::types::{MemoryInfo, DimmInfo};

pub fn collect_memory_info() -> MemoryInfo {
    let mut total_bytes: u64 = 0;

    // Collect memory information using smbios-lib
    let dimms = collect_memory_with_smbios();
    
    // Calculate total from collected DIMMs
    for dimm in &dimms {
        if let Some(size) = dimm.size_bytes {
            total_bytes += size;
        }
    }

    MemoryInfo {
        total_bytes: if total_bytes > 0 { Some(total_bytes) } else { None },
        dimms,
    }
}

fn collect_memory_with_smbios() -> Vec<DimmInfo> {
    let mut dimms = Vec::new();
    
    // Try to load SMBIOS data from the system
    let smbios_data = match SMBiosData::try_load_from_file("/sys/firmware/dmi/tables/DMI", None) {
        Ok(data) => data,
        Err(_) => {
            // If that fails, try reading the raw data and parsing it
            match fs::read("/sys/firmware/dmi/tables/DMI") {
                Ok(table_data) => {
                    SMBiosData::from_vec_and_version(table_data, None)
                },
                Err(_) => return dimms,
            }
        }
    };

    // Iterate through SMBIOS structures looking for memory devices
    for structure in smbios_data.iter() {
        match structure.defined_struct() {
            DefinedStruct::MemoryDevice(memory_device) => {
                // Only process memory devices that have actual memory installed
                if let Some(size_info) = memory_device.size() {
                    match size_info {
                        smbioslib::MemorySize::SeeExtendedSize => {
                            // Only process memory devices that have actual memory installed
                            let mut dimm = DimmInfo {
                                slot: None,
                                size_bytes: None,
                                mem_type: None,
                                speed_mt_s: None,
                                manufacturer: None,
                                serial_number: None,
                                part_number: None,
                            };

                            // Device locator (slot)
                            let device_locator = memory_device.device_locator();
                            if let Some(slot_name) = device_locator.to_utf8_lossy() {
                                if !slot_name.is_empty() {
                                    dimm.slot = Some(slot_name.to_string());
                                }
                            }

                            // Size - use extended size for large memory modules
                            if let Some(extended_size) = memory_device.extended_size() {
                                match extended_size {
                                    smbioslib::MemorySizeExtended::Megabytes(mb) if mb > 0 => {
                                        dimm.size_bytes = Some(mb as u64 * 1024 * 1024);
                                    },
                                    _ => {}
                                }
                            }

                            // Memory type
                            if let Some(mem_type) = memory_device.memory_type() {
                                let type_str = format!("{:?}", mem_type.value).to_uppercase();
                                if type_str != "UNKNOWN" {
                                    dimm.mem_type = Some(type_str);
                                }
                            }

                            // Speed - check configured and max speed
                            if let Some(config_speed) = memory_device.configured_memory_speed() {
                                match config_speed {
                                    smbioslib::MemorySpeed::MTs(mts) if mts > 0 => {
                                        dimm.speed_mt_s = Some(mts as u32);
                                    },
                                    _ => {}
                                }
                            } else if let Some(max_speed) = memory_device.speed() {
                                match max_speed {
                                    smbioslib::MemorySpeed::MTs(mts) if mts > 0 => {
                                        dimm.speed_mt_s = Some(mts as u32);
                                    },
                                    _ => {}
                                }
                            }

                            // Manufacturer
                            let manufacturer = memory_device.manufacturer();
                            if let Some(mfg_name) = manufacturer.to_utf8_lossy() {
                                if !mfg_name.is_empty() && mfg_name != "Not Specified" {
                                    dimm.manufacturer = Some(mfg_name.to_string());
                                }
                            }

                            // Serial number
                            let serial = memory_device.serial_number();
                            if let Some(serial_str) = serial.to_utf8_lossy() {
                                if !serial_str.is_empty() && serial_str != "Not Specified" {
                                    dimm.serial_number = Some(serial_str.to_string());
                                }
                            }

                            // Part number
                            let part_number = memory_device.part_number();
                            if let Some(part_str) = part_number.to_utf8_lossy() {
                                if !part_str.is_empty() && part_str != "Not Specified" {
                                    dimm.part_number = Some(part_str.to_string());
                                }
                            }

                            // Only add if we have valid size
                            if dimm.size_bytes.is_some() {
                                dimms.push(dimm);
                            }
                        },
                        smbioslib::MemorySize::Kilobytes(kb) if kb > 0 => {
                            let mut dimm = DimmInfo {
                                slot: None,
                                size_bytes: None,
                                mem_type: None,
                                speed_mt_s: None,
                                manufacturer: None,
                                serial_number: None,
                                part_number: None,
                            };

                            // Device locator (slot)
                            let device_locator = memory_device.device_locator();
                            if let Some(slot_name) = device_locator.to_utf8_lossy() {
                                if !slot_name.is_empty() {
                                    dimm.slot = Some(slot_name.to_string());
                                }
                            }

                            // Size calculation
                            let size_bytes = match size_info {
                                smbioslib::MemorySize::Kilobytes(kb) => kb as u64 * 1024,
                                smbioslib::MemorySize::Megabytes(mb) => {
                                    if mb == 0x7FFF {
                                        // Check extended size
                                        if let Some(ext_size) = memory_device.extended_size() {
                                            match ext_size {
                                                smbioslib::MemorySizeExtended::Megabytes(mb_ext) => mb_ext as u64 * 1024 * 1024,
                                                _ => mb as u64 * 1024 * 1024,
                                            }
                                        } else {
                                            mb as u64 * 1024 * 1024
                                        }
                                    } else {
                                        mb as u64 * 1024 * 1024
                                    }
                                },
                                _ => 0,
                            };
                            
                            if size_bytes > 0 {
                                dimm.size_bytes = Some(size_bytes);
                            }

                            // Memory type
                            if let Some(mem_type) = memory_device.memory_type() {
                                let type_str = format!("{:?}", mem_type).to_uppercase();
                                if type_str != "UNKNOWN" {
                                    dimm.mem_type = Some(type_str);
                                }
                            }

                            // Speed - check configured and max speed
                            if let Some(config_speed) = memory_device.configured_memory_speed() {
                                match config_speed {
                                    smbioslib::MemorySpeed::MTs(mts) if mts > 0 => {
                                        dimm.speed_mt_s = Some(mts as u32);
                                    },
                                    _ => {}
                                }
                            } else if let Some(max_speed) = memory_device.speed() {
                                match max_speed {
                                    smbioslib::MemorySpeed::MTs(mts) if mts > 0 => {
                                        dimm.speed_mt_s = Some(mts as u32);
                                    },
                                    _ => {}
                                }
                            }

                            // Manufacturer
                            let manufacturer = memory_device.manufacturer();
                            if let Some(mfg_name) = manufacturer.to_utf8_lossy() {
                                if !mfg_name.is_empty() && mfg_name != "Not Specified" {
                                    dimm.manufacturer = Some(mfg_name.to_string());
                                }
                            }

                            // Serial number
                            let serial = memory_device.serial_number();
                            if let Some(serial_str) = serial.to_utf8_lossy() {
                                if !serial_str.is_empty() && serial_str != "Not Specified" {
                                    dimm.serial_number = Some(serial_str.to_string());
                                }
                            }

                            // Part number
                            let part_num = memory_device.part_number();
                            if let Some(part_str) = part_num.to_utf8_lossy() {
                                if !part_str.is_empty() && part_str != "Not Specified" {
                                    dimm.part_number = Some(part_str.to_string());
                                }
                            }

                            dimms.push(dimm);
                        },
                        smbioslib::MemorySize::Megabytes(mb) if mb > 0 => {
                            let mut dimm = DimmInfo {
                                slot: None,
                                size_bytes: None,
                                mem_type: None,
                                speed_mt_s: None,
                                manufacturer: None,
                                serial_number: None,
                                part_number: None,
                            };

                            // Device locator (slot)
                            let device_locator = memory_device.device_locator();
                            if let Some(slot_name) = device_locator.to_utf8_lossy() {
                                if !slot_name.is_empty() {
                                    dimm.slot = Some(slot_name.to_string());
                                }
                            }

                            // Size calculation for megabytes
                            let size_bytes = if mb == 0x7FFF {
                                // Check extended size
                                if let Some(ext_size) = memory_device.extended_size() {
                                    match ext_size {
                                        smbioslib::MemorySizeExtended::Megabytes(mb_ext) => mb_ext as u64 * 1024 * 1024,
                                        _ => mb as u64 * 1024 * 1024,
                                    }
                                } else {
                                    mb as u64 * 1024 * 1024
                                }
                            } else {
                                mb as u64 * 1024 * 1024
                            };
                            
                            dimm.size_bytes = Some(size_bytes);

                            // Memory type
                            if let Some(mem_type) = memory_device.memory_type() {
                                let type_str = format!("{:?}", mem_type).to_uppercase();
                                if type_str != "UNKNOWN" {
                                    dimm.mem_type = Some(type_str);
                                }
                            }

                            // Speed - check configured and max speed
                            if let Some(config_speed) = memory_device.configured_memory_speed() {
                                match config_speed {
                                    smbioslib::MemorySpeed::MTs(mts) if mts > 0 => {
                                        dimm.speed_mt_s = Some(mts as u32);
                                    },
                                    _ => {}
                                }
                            } else if let Some(max_speed) = memory_device.speed() {
                                match max_speed {
                                    smbioslib::MemorySpeed::MTs(mts) if mts > 0 => {
                                        dimm.speed_mt_s = Some(mts as u32);
                                    },
                                    _ => {}
                                }
                            }

                            // Manufacturer
                            let manufacturer = memory_device.manufacturer();
                            if let Some(mfg_name) = manufacturer.to_utf8_lossy() {
                                if !mfg_name.is_empty() && mfg_name != "Not Specified" {
                                    dimm.manufacturer = Some(mfg_name.to_string());
                                }
                            }

                            // Serial number
                            let serial = memory_device.serial_number();
                            if let Some(serial_str) = serial.to_utf8_lossy() {
                                if !serial_str.is_empty() && serial_str != "Not Specified" {
                                    dimm.serial_number = Some(serial_str.to_string());
                                }
                            }

                            // Part number
                            let part_num = memory_device.part_number();
                            if let Some(part_str) = part_num.to_utf8_lossy() {
                                if !part_str.is_empty() && part_str != "Not Specified" {
                                    dimm.part_number = Some(part_str.to_string());
                                }
                            }

                            dimms.push(dimm);
                        },
                        _ => continue,
                    }
                }
            }
            _ => continue,
        }
    }
    
    dimms
}
