use crate::hardware::types::{HashcatInfo, HashcatTestResult, HashcatDevice};
use std::process::Command;

/// Get Hashcat installation information and version
pub fn collect_hashcat_info() -> HashcatInfo {
    let mut info = HashcatInfo {
        hashcat_version: None,
        hashcat_available: false,
        opencl_available: false,
        cuda_available: false,
        num_devices: 0,
        devices: Vec::new(),
        error: None,
    };
    
    // Check if hashcat is available
    if let Ok(output) = Command::new("which").arg("hashcat").output() {
        if !output.status.success() {
            info.error = Some("hashcat not found. Please install hashcat.".to_string());
            return info;
        }
        info.hashcat_available = true;
    }
    
    // Get hashcat version
    if let Ok(output) = Command::new("hashcat").arg("--version").output() {
        if output.status.success() {
            let version_str = String::from_utf8_lossy(&output.stdout);
            info.hashcat_version = Some(version_str.trim().to_string());
        }
    }
    
    // Get device information using hashcat -I
    if let Ok(output) = Command::new("hashcat").arg("-I").output() {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            parse_hashcat_devices(&output_str, &mut info);
        }
    }
    
    info
}

/// Parse hashcat device information from -I output
fn parse_hashcat_devices(output: &str, info: &mut HashcatInfo) {
    let mut current_device_id = None;
    let mut current_device_name = None;
    let mut current_device_type = None;
    
    for line in output.lines() {
        let trimmed = line.trim();
        
        // Check for OpenCL/CUDA availability
        if trimmed.contains("OpenCL") {
            info.opencl_available = true;
        }
        if trimmed.contains("CUDA") {
            info.cuda_available = true;
        }
        
        // Parse device entries
        if trimmed.starts_with("Backend Device ID") || trimmed.starts_with("Device ID") {
            if let Some(id_str) = trimmed.split('#').nth(1) {
                if let Ok(id) = id_str.split_whitespace().next().unwrap_or("").parse::<u32>() {
                    current_device_id = Some(id);
                }
            }
        }
        
        if trimmed.starts_with("Name") && trimmed.contains(':') {
            if let Some(name) = trimmed.split(':').nth(1) {
                current_device_name = Some(name.trim().to_string());
            }
        }
        
        if trimmed.starts_with("Type") && trimmed.contains(':') {
            if let Some(dev_type) = trimmed.split(':').nth(1) {
                current_device_type = Some(dev_type.trim().to_string());
            }
        }
        
        // When we have all info for a device, add it
        if let (Some(id), Some(name), Some(dev_type)) = 
            (current_device_id, current_device_name.clone(), current_device_type.clone()) {
            
            info.devices.push(HashcatDevice {
                device_id: id,
                device_name: name,
                device_type: dev_type,
                opencl_version: None,
                cuda_version: None,
            });
            
            info.num_devices += 1;
            
            // Reset for next device
            current_device_id = None;
            current_device_name = None;
            current_device_type = None;
        }
    }
}

/// Run a hashcat benchmark
pub fn run_hashcat_benchmark(hash_types: Vec<String>, device_ids: Option<Vec<u32>>) 
    -> Result<Vec<HashcatTestResult>, Box<dyn std::error::Error>> {
    
    let mut results = Vec::new();
    
    // Check if hashcat is available
    if !Command::new("which")
        .arg("hashcat")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Err("hashcat not found. Please install hashcat.".into());
    }
    
    for hash_type in hash_types {
        let result = run_single_benchmark(&hash_type, device_ids.as_ref())?;
        results.push(result);
    }
    
    Ok(results)
}

/// Run a single hashcat benchmark for a specific hash type
fn run_single_benchmark(hash_type: &str, device_ids: Option<&Vec<u32>>) 
    -> Result<HashcatTestResult, Box<dyn std::error::Error>> {
    
    let mut result = HashcatTestResult {
        test_type: "benchmark".to_string(),
        hash_type: Some(hash_type.to_string()),
        device_ids: device_ids.map(|v| v.clone()).unwrap_or_default(),
        success: false,
        hash_speed: None,
        time_seconds: None,
        recovered: None,
        total: None,
        error: None,
        raw_output: None,
    };
    
    // Build command
    let mut cmd = Command::new("hashcat");
    cmd.arg("-b");
    cmd.arg("-m");
    cmd.arg(hash_type);
    
    // Specify devices if provided
    if let Some(devices) = device_ids {
        if !devices.is_empty() {
            let device_str = devices.iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",");
            cmd.arg("-d");
            cmd.arg(device_str);
        }
    }
    
    // Run the benchmark
    let start_time = std::time::Instant::now();
    let output = cmd.output()?;
    let elapsed = start_time.elapsed().as_secs_f64();
    
    result.time_seconds = Some(elapsed);
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    result.raw_output = Some(output_str.to_string());
    
    if output.status.success() {
        result.success = true;
        
        // Parse benchmark results for hash speed
        if let Some(speed) = parse_benchmark_speed(&output_str) {
            result.hash_speed = Some(speed);
        }
    } else {
        let error_str = String::from_utf8_lossy(&output.stderr);
        result.error = Some(format!("Benchmark failed: {}", error_str));
    }
    
    Ok(result)
}

/// Run a hashcat dictionary attack test
pub fn run_hashcat_test(
    hash_type: &str,
    hash_file: &str,
    wordlist: &str,
    device_ids: Option<Vec<u32>>,
) -> Result<HashcatTestResult, Box<dyn std::error::Error>> {
    
    let mut result = HashcatTestResult {
        test_type: "dictionary".to_string(),
        hash_type: Some(hash_type.to_string()),
        device_ids: device_ids.clone().unwrap_or_default(),
        success: false,
        hash_speed: None,
        time_seconds: None,
        recovered: None,
        total: None,
        error: None,
        raw_output: None,
    };
    
    // Check if hashcat is available
    if !Command::new("which")
        .arg("hashcat")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        result.error = Some("hashcat not found. Please install hashcat.".to_string());
        return Ok(result);
    }
    
    // Verify files exist
    if !std::path::Path::new(hash_file).exists() {
        result.error = Some(format!("Hash file not found: {}", hash_file));
        return Ok(result);
    }
    
    if !std::path::Path::new(wordlist).exists() {
        result.error = Some(format!("Wordlist file not found: {}", wordlist));
        return Ok(result);
    }
    
    // Build command
    let mut cmd = Command::new("hashcat");
    cmd.arg("-m");
    cmd.arg(hash_type);
    cmd.arg("-a");
    cmd.arg("0"); // Dictionary attack
    cmd.arg(hash_file);
    cmd.arg(wordlist);
    
    // Specify devices if provided
    if let Some(devices) = &device_ids {
        if !devices.is_empty() {
            let device_str = devices.iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",");
            cmd.arg("-d");
            cmd.arg(device_str);
        }
    }
    
    // Add --show flag to display results
    cmd.arg("--quiet");
    
    // Run the test
    let start_time = std::time::Instant::now();
    let output = cmd.output()?;
    let elapsed = start_time.elapsed().as_secs_f64();
    
    result.time_seconds = Some(elapsed);
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let error_str = String::from_utf8_lossy(&output.stderr);
    result.raw_output = Some(format!("{}\n{}", output_str, error_str));
    
    // Parse results
    if output.status.success() || output_str.contains("Recovered") {
        result.success = true;
        
        // Parse recovered/total hashes
        if let Some((recovered, total)) = parse_recovered_hashes(&output_str) {
            result.recovered = Some(recovered);
            result.total = Some(total);
        }
        
        // Parse hash speed
        if let Some(speed) = parse_hash_speed(&output_str) {
            result.hash_speed = Some(speed);
        }
    } else {
        result.error = Some(format!("Test failed: {}", error_str));
    }
    
    Ok(result)
}

/// Parse benchmark speed from hashcat output (H/s)
fn parse_benchmark_speed(output: &str) -> Option<f64> {
    // Look for lines like: "Speed.#1.........:   123.4 MH/s"
    for line in output.lines() {
        if line.contains("Speed") && (line.contains("H/s") || line.contains("kH/s") || 
                                      line.contains("MH/s") || line.contains("GH/s")) {
            if let Some(speed_part) = line.split(':').nth(1) {
                let parts: Vec<&str> = speed_part.trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(speed) = parts[0].parse::<f64>() {
                        let unit = parts[1];
                        let multiplier = match unit {
                            "H/s" => 1.0,
                            "kH/s" => 1_000.0,
                            "MH/s" => 1_000_000.0,
                            "GH/s" => 1_000_000_000.0,
                            "TH/s" => 1_000_000_000_000.0,
                            _ => 1.0,
                        };
                        return Some(speed * multiplier);
                    }
                }
            }
        }
    }
    None
}

/// Parse hash speed from hashcat test output
fn parse_hash_speed(output: &str) -> Option<f64> {
    parse_benchmark_speed(output)
}

/// Parse recovered hashes from hashcat output
fn parse_recovered_hashes(output: &str) -> Option<(u32, u32)> {
    // Look for lines like: "Recovered........: 5/10 (50.00%)"
    for line in output.lines() {
        if line.contains("Recovered") && line.contains('/') {
            if let Some(ratio_part) = line.split(':').nth(1) {
                let parts: Vec<&str> = ratio_part.trim().split('/').collect();
                if parts.len() >= 2 {
                    if let Ok(recovered) = parts[0].trim().parse::<u32>() {
                        let total_part = parts[1].split_whitespace().next().unwrap_or("");
                        if let Ok(total) = total_part.parse::<u32>() {
                            return Some((recovered, total));
                        }
                    }
                }
            }
        }
    }
    None
}
