use std::fs;
use std::collections::HashMap;
use smbioslib::*;
use crate::hardware::types::{CpuInfo, CpuSocket};

fn get_cache_size_by_handle(smbios: &SMBiosData, handle: Handle) -> Option<u32> {
    for structure in smbios.iter() {
        if let DefinedStruct::CacheInformation(cache) = structure.defined_struct() {
            if structure.header.handle() == handle {
                if let Some(size_data) = cache.installed_size() {
                    // Extract the cache size in KB by parsing the debug representation
                    let debug_str = format!("{:?}", size_data);
                    if debug_str.starts_with("Kilobytes(") && debug_str.ends_with(")") {
                        let num_str = &debug_str[10..debug_str.len()-1];
                        return num_str.parse::<u32>().ok();
                    }
                }
                break;
            }
        }
    }
    None
}

pub fn collect_cpu_info() -> CpuInfo {
    let mut cpu_data: HashMap<u32, CpuSocket> = HashMap::new();
    let mut socket_count = 0u32;

    // Collect CPU information using smbios-lib
    collect_with_smbios(&mut cpu_data);

    // Calculate totals based on detected CPUs
    let mut total_cores = 0u32;
    let mut total_threads = 0u32;
    
    for cpu in cpu_data.values() {
        if let Some(cores) = cpu.num_cores {
            total_cores += cores;
        }
        if let Some(threads) = cpu.num_threads {
            total_threads += threads;
        }
    }
    
    socket_count = cpu_data.len() as u32;

    let mut cpus: Vec<CpuSocket> = cpu_data.into_values().collect();
    cpus.sort_by_key(|cpu| cpu.socket);

    CpuInfo {
        sockets: if socket_count > 0 { Some(socket_count) } else { None },
        cores: if total_cores > 0 { Some(total_cores) } else { None },
        threads: if total_threads > 0 { Some(total_threads) } else { None },
        cpus,
    }
}

fn collect_with_smbios(cpu_data: &mut HashMap<u32, CpuSocket>) {
    // Try to load SMBIOS data from the system
    let smbios_data = match SMBiosData::try_load_from_file("/sys/firmware/dmi/tables/DMI", None) {
        Ok(data) => data,
        Err(_) => {
            // If that fails, try reading the raw data and parsing it
            match fs::read("/sys/firmware/dmi/tables/DMI") {
                Ok(table_data) => {
                    SMBiosData::from_vec_and_version(table_data, None)
                },
                Err(_) => return,
            }
        }
    };

    let mut socket_index = 0u32;
    
    // Iterate through SMBIOS structures looking for processor information
    for structure in smbios_data.iter() {
        match structure.defined_struct() {
            DefinedStruct::ProcessorInformation(processor) => {
                let mut cpu = CpuSocket {
                    socket: socket_index,
                    manufacturer: None,
                    model_name: None,
                    num_cores: None,
                    num_threads: None,
                    capacity_mhz: None,
                    slot: None,
                    l1_cache_kb: None,
                    l2_cache_kb: None,
                    l3_cache_kb: None,
                };

                // Socket designation
                if let Some(socket_str) = processor.socket_designation().to_utf8_lossy() {
                    if !socket_str.is_empty() {
                        cpu.slot = Some(socket_str.to_string());
                    }
                }
                
                // Manufacturer
                if let Some(manufacturer_str) = processor.processor_manufacturer().to_utf8_lossy() {
                    if !manufacturer_str.is_empty() {
                        cpu.manufacturer = Some(manufacturer_str.to_string());
                    }
                }
                
                // Processor version/model name
                if let Some(version_str) = processor.processor_version().to_utf8_lossy() {
                    let trimmed_version = version_str.trim();
                    if !trimmed_version.is_empty() && trimmed_version != "Not Specified" {
                        cpu.model_name = Some(trimmed_version.to_string());
                    }
                }

                // Core and thread count
                if let Some(thread_count) = processor.thread_count() {
                    match thread_count {
                        smbioslib::ThreadCount::Count(count) if count > 0 => {
                            cpu.num_threads = Some(count as u32);
                        },
                        _ => {}
                    }
                }

                if let Some(core_count) = processor.core_count() {
                    match core_count {
                        smbioslib::CoreCount::Count(count) if count > 0 => {
                            cpu.num_cores = Some(count as u32);
                        },
                        _ => {}
                    }
                }
                
                // Current speed and max speed
                if let Some(current_speed) = processor.current_speed() {
                    match current_speed {
                        smbioslib::ProcessorSpeed::MHz(mhz) if mhz > 0 => {
                            cpu.capacity_mhz = Some(mhz as u32);
                        },
                        _ => {}
                    }
                }

                // Max speed
                if let Some(max_speed) = processor.max_speed() {
                    match max_speed {
                        smbioslib::ProcessorSpeed::MHz(mhz) if mhz > 0 => {
                            // Store max speed in capacity_mhz if current speed wasn't available
                            if cpu.capacity_mhz.is_none() {
                                cpu.capacity_mhz = Some(mhz as u32);
                            }
                        },
                        _ => {}
                    }
                }

                // Get cache sizes using processor's cache handles
                if let Some(l1_handle) = processor.l1cache_handle() {
                    if let Some(l1_size) = get_cache_size_by_handle(&smbios_data, l1_handle) {
                        cpu.l1_cache_kb = Some(l1_size);
                    }
                }
                
                if let Some(l2_handle) = processor.l2cache_handle() {
                    if let Some(l2_size) = get_cache_size_by_handle(&smbios_data, l2_handle) {
                        cpu.l2_cache_kb = Some(l2_size);
                    }
                }
                
                if let Some(l3_handle) = processor.l3cache_handle() {
                    if let Some(l3_size) = get_cache_size_by_handle(&smbios_data, l3_handle) {
                        cpu.l3_cache_kb = Some(l3_size);
                    }
                }

                cpu_data.insert(socket_index, cpu);
                socket_index += 1;
            }
            _ => continue,
        }
    }
}