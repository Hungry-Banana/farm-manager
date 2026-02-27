use std::process::Command;
use std::fs;
use std::path::Path;
use pciid_parser::Database;
use crate::hardware::types::GpuInfo;

pub fn collect_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    // Scan PCI devices in /sys/bus/pci/devices for GPU devices
    let pci_devices_path = Path::new("/sys/bus/pci/devices");
    if let Ok(entries) = fs::read_dir(pci_devices_path) {
        for entry in entries.flatten() {
            let device_path = entry.path();
            
            // Check if this is a GPU device by reading the class
            if let Some(class_id) = read_pci_class(&device_path) {
                // GPU classes: 0x030000 (VGA), 0x030200 (3D), 0x038000 (Display)
                if is_gpu_class(&class_id) {
                    if let Some(gpu) = create_gpu_info(&device_path) {
                        gpus.push(gpu);
                    }
                }
            }
        }
    }

    // Also try vendor-specific tools for enhanced information
    enhance_gpus_with_tools(&mut gpus);

    gpus
}

fn read_pci_class(device_path: &Path) -> Option<String> {
    let class_file = device_path.join("class");
    fs::read_to_string(class_file)
        .ok()
        .map(|s| s.trim().to_string())
}

fn is_gpu_class(class_id: &str) -> bool {
    // GPU PCI classes:
    // 0x030000 - VGA compatible controller
    // 0x030200 - 3D controller  
    // 0x038000 - Display controller
    match class_id {
        c if c.starts_with("0x0300") => true,  // VGA and 3D controllers
        c if c.starts_with("0x0380") => true,  // Display controllers
        _ => false,
    }
}

fn create_gpu_info(device_path: &Path) -> Option<GpuInfo> {
    // Get PCI address from device path
    let pci_address = device_path.file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.to_string())?;

    // Read vendor and device IDs
    let vendor_id = read_hex_file(&device_path.join("vendor"))?;
    let device_id = read_hex_file(&device_path.join("device"))?;

    // Look up vendor and device names using PCI database
    let (vendor_name, device_name) = lookup_pci_names(vendor_id, device_id)?;

    Some(GpuInfo {
        vendor: Some(vendor_name),
        model: Some(device_name),
        pci_address: Some(pci_address),
        vram_mb: None,
        driver_version: None,
        uuid: None,
    })
}

fn read_hex_file(path: &Path) -> Option<u16> {
    let content = fs::read_to_string(path).ok()?;
    let hex_str = content.trim().strip_prefix("0x").unwrap_or(content.trim());
    u16::from_str_radix(hex_str, 16).ok()
}

fn lookup_pci_names(vendor_id: u16, device_id: u16) -> Option<(String, String)> {
    let db = Database::read().ok()?;
    
    // Get vendor
    let vendor = db.vendors.get(&vendor_id)?;
    let vendor_name = vendor.name.clone();
    
    // Get device - fallback if device not found
    let device_name = vendor.devices.get(&device_id)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| format!("Unknown Device [0x{:04x}]", device_id));
    
    Some((vendor_name, device_name))
}
fn enhance_gpus_with_tools(gpus: &mut Vec<GpuInfo>) {
    // Try to enhance with vendor-specific tools for VRAM, driver version, and UUID
    for gpu in gpus.iter_mut() {
        if let Some(vendor) = &gpu.vendor {
            if vendor.to_lowercase().contains("nvidia") {
                enhance_nvidia_gpu(gpu);
            } else if vendor.to_lowercase().contains("amd") || vendor.to_lowercase().contains("ati") {
                enhance_amd_gpu(gpu);
            }
        }
    }
}

fn enhance_nvidia_gpu(gpu: &mut GpuInfo) {
    // Try nvidia-smi for VRAM, driver version, and UUID
    if let Ok(output) = Command::new("nvidia-smi")
        .args(&["--query-gpu=name,memory.total,driver_version,uuid", 
                "--format=csv,noheader,nounits"])
        .output()
    {
        if output.status.success() {
            let nvidia_output = String::from_utf8_lossy(&output.stdout);
            for line in nvidia_output.lines() {
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 4 {
                    // Try to match this GPU by checking if the model contains key parts
                    if let Some(model) = &gpu.model {
                        if gpu_models_match(&parts[0], model) {
                            if let Ok(vram) = parts[1].parse::<u32>() {
                                gpu.vram_mb = Some(vram);
                            }
                            gpu.driver_version = Some(parts[2].to_string());
                            gpu.uuid = Some(parts[3].to_string());
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn enhance_amd_gpu(gpu: &mut GpuInfo) {
    // Try rocm-smi for AMD GPUs - focus on driver version and memory info
    if let Ok(output) = Command::new("rocm-smi")
        .args(&["--showproductname", "--showmeminfo"])
        .output()
    {
        if output.status.success() {
            let rocm_output = String::from_utf8_lossy(&output.stdout);
            
            // Basic parsing for memory information
            for line in rocm_output.lines() {
                if line.contains("Memory") && line.contains("MB") {
                    if let Some(mem_str) = extract_number_from_line(line) {
                        if let Ok(vram) = mem_str.parse::<u32>() {
                            gpu.vram_mb = Some(vram);
                        }
                    }
                }
            }
        }
    }
}

fn gpu_models_match(nvidia_name: &str, pci_name: &str) -> bool {
    // Simple matching - check if key parts of the GPU name match
    let nvidia_lower = nvidia_name.to_lowercase();
    let nvidia_parts: Vec<&str> = nvidia_lower.split_whitespace().collect();
    let pci_lower = pci_name.to_lowercase();
    let pci_parts: Vec<&str> = pci_lower.split_whitespace().collect();
    
    // Look for common model identifiers
    for nvidia_part in &nvidia_parts {
        for pci_part in &pci_parts {
            if nvidia_part.len() > 3 && pci_part.contains(nvidia_part) {
                return true;
            }
        }
    }
    false
}

fn extract_number_from_line(line: &str) -> Option<&str> {
    line.split_whitespace()
        .find(|s| s.chars().all(|c| c.is_ascii_digit()))
}