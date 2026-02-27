use nvml_wrapper::Nvml;
use crate::hardware::types::{NcclInfo, NcclTestResult, NcclGpuResult};
use std::process::Command;

/// Get NCCL installation information and version
pub fn collect_nccl_info() -> NcclInfo {
    let mut info = NcclInfo {
        nccl_version: None,
        cuda_version: None,
        num_gpus: 0,
        nccl_available: false,
        nccl_tests_available: false,
        error: None,
    };
    
    // Try to get GPU count using NVML
    if let Ok(nvml) = Nvml::init() {
        if let Ok(count) = nvml.device_count() {
            info.num_gpus = count;
        }
    }
    
    // Check for NCCL tests binaries
    let nccl_test_binaries = [
        "nccl-tests",
        "all_reduce_perf",
        "all_gather_perf",
        "broadcast_perf",
        "reduce_scatter_perf",
    ];
    
    for binary in &nccl_test_binaries {
        if let Ok(output) = Command::new("which").arg(binary).output() {
            if output.status.success() {
                info.nccl_tests_available = true;
                break;
            }
        }
    }
    
    // Try to get CUDA version
    if let Ok(output) = Command::new("nvcc").arg("--version").output() {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if line.contains("release") {
                    if let Some(version) = extract_cuda_version(line) {
                        info.cuda_version = Some(version);
                        break;
                    }
                }
            }
        }
    }
    
    // Try to detect NCCL library
    let nccl_lib_paths = [
        "/usr/local/cuda/lib64/libnccl.so",
        "/usr/lib/x86_64-linux-gnu/libnccl.so",
        "/usr/lib64/libnccl.so",
        "/opt/cuda/lib64/libnccl.so",
    ];
    
    for path in &nccl_lib_paths {
        if std::path::Path::new(path).exists() {
            info.nccl_available = true;
            
            // Try to get version from library
            if let Some(version) = get_nccl_version_from_lib(path) {
                info.nccl_version = Some(version);
            }
            break;
        }
    }
    
    // Alternative: try to get version from pkg-config
    if info.nccl_version.is_none() {
        if let Ok(output) = Command::new("pkg-config")
            .args(&["--modversion", "nccl"])
            .output()
        {
            if output.status.success() {
                info.nccl_version = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }
    }
    
    info
}

/// Run NCCL test
pub fn run_nccl_test(test_type: &str, size: &str, iterations: u32) -> Result<NcclTestResult, Box<dyn std::error::Error>> {
    let nvml = Nvml::init()?;
    let device_count = nvml.device_count()?;
    
    if device_count == 0 {
        return Err("No NVIDIA GPUs found".into());
    }
    
    let size_bytes = parse_size(size)?;
    
    let mut result = NcclTestResult {
        test_type: test_type.to_string(),
        size_bytes,
        iterations,
        num_gpus: device_count,
        success: false,
        time_us: None,
        bandwidth_gbps: None,
        bus_bandwidth_gbps: None,
        error: None,
        gpu_results: Vec::new(),
    };
    
    // Collect GPU information
    for i in 0..device_count {
        if let Ok(device) = nvml.device_by_index(i) {
            let name = device.name().unwrap_or_else(|_| format!("GPU {}", i));
            result.gpu_results.push(NcclGpuResult {
                device_index: i,
                device_name: name,
                in_place: true,
                out_of_place: true,
            });
        }
    }
    
    // Map test types to NCCL test binary names
    let test_binary = match test_type.to_lowercase().as_str() {
        "all-reduce" | "allreduce" => "all_reduce_perf",
        "broadcast" => "broadcast_perf",
        "reduce-scatter" | "reducescatter" => "reduce_scatter_perf",
        "all-gather" | "allgather" => "all_gather_perf",
        "reduce" => "reduce_perf",
        "all-to-all" | "alltoall" => "alltoall_perf",
        "bandwidth" => "all_reduce_perf", // Use all-reduce for bandwidth test
        _ => "all_reduce_perf",
    };
    
    // Try to run the NCCL test binary
    let test_result = Command::new(test_binary)
        .args(&[
            "-b", &size,  // min size
            "-e", &size,  // max size
            "-f", "2",    // size multiplication factor
            "-g", &device_count.to_string(),  // number of GPUs
            "-n", &iterations.to_string(),    // number of iterations
        ])
        .output();
    
    match test_result {
        Ok(output) => {
            if output.status.success() {
                result.success = true;
                let output_str = String::from_utf8_lossy(&output.stdout);
                
                // Parse the output for bandwidth and timing information
                if let Some((time, bandwidth, bus_bw)) = parse_nccl_output(&output_str) {
                    result.time_us = Some(time);
                    result.bandwidth_gbps = Some(bandwidth);
                    result.bus_bandwidth_gbps = Some(bus_bw);
                }
            } else {
                result.error = Some(format!(
                    "Test failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
        Err(e) => {
            // If test binary not found, try a simple NVML-based test
            if e.kind() == std::io::ErrorKind::NotFound {
                result.error = Some(format!(
                    "NCCL test binary '{}' not found. Install nccl-tests package for full testing. \
                    To install: git clone https://github.com/NVIDIA/nccl-tests.git && cd nccl-tests && make",
                    test_binary
                ));
                
                // Still provide basic info about GPUs
                result.success = false;
            } else {
                result.error = Some(format!("Failed to run test: {}", e));
            }
        }
    }
    
    Ok(result)
}

/// Parse size string (e.g., "32M", "1G", "512K") to bytes
fn parse_size(size: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let size = size.trim().to_uppercase();
    
    let (number, multiplier) = if size.ends_with('K') {
        (size.trim_end_matches('K'), 1024u64)
    } else if size.ends_with('M') {
        (size.trim_end_matches('M'), 1024u64 * 1024)
    } else if size.ends_with('G') {
        (size.trim_end_matches('G'), 1024u64 * 1024 * 1024)
    } else {
        (size.as_str(), 1u64)
    };
    
    let num: u64 = number.parse()?;
    Ok(num * multiplier)
}

/// Parse NCCL test output to extract performance metrics
fn parse_nccl_output(output: &str) -> Option<(f64, f64, f64)> {
    // NCCL test output format typically looks like:
    // #       size         count      type   redop     time   algbw   busbw  error
    // #    (bytes)    (elements)                       (us)  (GB/s)  (GB/s)
    //      33554432       8388608     float     sum   1234.5   27.2   51.0  N/A
    
    for line in output.lines() {
        // Skip comments and headers
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 8 {
            // Try to parse time (us), algbw (GB/s), and busbw (GB/s)
            if let (Ok(time), Ok(algbw), Ok(busbw)) = (
                parts[5].parse::<f64>(),
                parts[6].parse::<f64>(),
                parts[7].parse::<f64>(),
            ) {
                return Some((time, algbw, busbw));
            }
        }
    }
    
    None
}

/// Extract CUDA version from nvcc output
fn extract_cuda_version(line: &str) -> Option<String> {
    // Example: "Cuda compilation tools, release 11.8, V11.8.89"
    if let Some(start) = line.find("release") {
        let version_part = &line[start + 7..];
        if let Some(end) = version_part.find(',') {
            return Some(version_part[..end].trim().to_string());
        }
    }
    None
}

/// Get NCCL version from library using strings command
fn get_nccl_version_from_lib(lib_path: &str) -> Option<String> {
    if let Ok(output) = Command::new("strings")
        .arg(lib_path)
        .output()
    {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                // Look for version strings like "2.18.3" or "NCCL version 2.18.3"
                if line.contains("NCCL") && line.contains("version") {
                    if let Some(version) = extract_version_number(line) {
                        return Some(version);
                    }
                }
                // Also check for standalone version patterns
                if line.chars().filter(|c| *c == '.').count() == 2 {
                    if let Some(version) = extract_version_number(line) {
                        if version.split('.').count() == 3 {
                            return Some(version);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extract version number from string
fn extract_version_number(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    for part in parts {
        // Check if this looks like a version (e.g., "2.18.3")
        if part.chars().filter(|c| *c == '.').count() >= 1 {
            let mut version = String::new();
            for ch in part.chars() {
                if ch.is_ascii_digit() || ch == '.' {
                    version.push(ch);
                } else if !version.is_empty() {
                    break;
                }
            }
            if !version.is_empty() && version.chars().filter(|c| *c == '.').count() >= 1 {
                return Some(version);
            }
        }
    }
    None
}
