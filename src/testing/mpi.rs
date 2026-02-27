use crate::hardware::types::{MpiInfo, MpiTestResult};
use std::process::Command;
use sysinfo::System;

/// Get MPI installation information and version
pub fn collect_mpi_info() -> MpiInfo {
    let mut info = MpiInfo {
        mpi_version: None,
        mpi_implementation: None,
        mpi_available: false,
        mpirun_available: false,
        num_cpus: 0,
        mpi_benchmark_available: false,
        error: None,
    };
    
    // Get CPU count
    let mut sys = System::new_all();
    sys.refresh_all();
    info.num_cpus = sys.cpus().len() as u32;
    
    // Check for mpirun/mpiexec
    for binary in &["mpirun", "mpiexec"] {
        if let Ok(output) = Command::new("which").arg(binary).output() {
            if output.status.success() {
                info.mpirun_available = true;
                break;
            }
        }
    }
    
    // Get MPI version from mpirun
    if info.mpirun_available {
        if let Ok(output) = Command::new("mpirun").arg("--version").output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                
                // Parse implementation and version
                let (implementation, version) = parse_mpi_version(&output_str);
                info.mpi_implementation = implementation;
                info.mpi_version = version;
                info.mpi_available = true;
            }
        }
    }
    
    // Check for MPI benchmarks (OSU Micro-Benchmarks, IMB, etc.)
    let benchmark_binaries = [
        "osu_latency",
        "osu_bw",
        "osu_allreduce",
        "IMB-MPI1",
        "mpptest",
    ];
    
    for binary in &benchmark_binaries {
        if let Ok(output) = Command::new("which").arg(binary).output() {
            if output.status.success() {
                info.mpi_benchmark_available = true;
                break;
            }
        }
    }
    
    info
}

/// Run MPI test
pub fn run_mpi_test(
    test_type: &str,
    num_processes: u32,
    size: &str,
    iterations: u32,
) -> Result<MpiTestResult, Box<dyn std::error::Error>> {
    let size_bytes = parse_size(size)?;
    
    let mut result = MpiTestResult {
        test_type: test_type.to_string(),
        num_processes,
        size_bytes,
        iterations,
        success: false,
        latency_us: None,
        bandwidth_mbps: None,
        min_latency_us: None,
        max_latency_us: None,
        avg_latency_us: None,
        error: None,
        raw_output: None,
    };
    
    // Check if mpirun is available
    if !Command::new("which")
        .arg("mpirun")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        result.error = Some("mpirun not found. Please install an MPI implementation (OpenMPI, MPICH, Intel MPI, etc.)".to_string());
        return Ok(result);
    }
    
    // Try to use OSU Micro-Benchmarks if available
    if let Some(test_result) = try_osu_benchmark(test_type, num_processes, size_bytes, iterations) {
        return Ok(test_result);
    }
    
    // Try to use Intel MPI Benchmarks if available
    if let Some(test_result) = try_imb_benchmark(test_type, num_processes, size_bytes, iterations) {
        return Ok(test_result);
    }
    
    // Fallback: Create and run a simple MPI test program
    run_custom_mpi_test(test_type, num_processes, size_bytes, iterations)
}

/// Try to run OSU Micro-Benchmarks
fn try_osu_benchmark(
    test_type: &str,
    num_processes: u32,
    size_bytes: u64,
    iterations: u32,
) -> Option<MpiTestResult> {
    let benchmark_name = match test_type.to_lowercase().as_str() {
        "ping-pong" | "latency" => "osu_latency",
        "bandwidth" | "bw" => "osu_bw",
        "all-reduce" | "allreduce" => "osu_allreduce",
        "broadcast" => "osu_bcast",
        "barrier" => "osu_barrier",
        _ => return None,
    };
    
    // Check if benchmark exists
    if !Command::new("which")
        .arg(benchmark_name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return None;
    }
    
    // Run the benchmark
    let output = Command::new("mpirun")
        .args(&[
            "-n", &num_processes.to_string(),
            "--allow-run-as-root",  // Some systems require this
            benchmark_name,
        ])
        .output()
        .ok()?;
    
    let mut result = MpiTestResult {
        test_type: test_type.to_string(),
        num_processes,
        size_bytes,
        iterations,
        success: output.status.success(),
        latency_us: None,
        bandwidth_mbps: None,
        min_latency_us: None,
        max_latency_us: None,
        avg_latency_us: None,
        error: None,
        raw_output: None,
    };
    
    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        result.raw_output = Some(output_str.to_string());
        
        // Parse OSU benchmark output
        parse_osu_output(&output_str, &mut result, size_bytes);
    } else {
        result.error = Some(String::from_utf8_lossy(&output.stderr).to_string());
    }
    
    Some(result)
}

/// Try to run Intel MPI Benchmarks
fn try_imb_benchmark(
    test_type: &str,
    num_processes: u32,
    _size_bytes: u64,
    _iterations: u32,
) -> Option<MpiTestResult> {
    let benchmark_name = "IMB-MPI1";
    
    // Check if benchmark exists
    if !Command::new("which")
        .arg(benchmark_name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return None;
    }
    
    let imb_test = match test_type.to_lowercase().as_str() {
        "ping-pong" | "latency" => "PingPong",
        "all-reduce" | "allreduce" => "Allreduce",
        "broadcast" => "Bcast",
        "barrier" => "Barrier",
        _ => return None,
    };
    
    // Run IMB
    let output = Command::new("mpirun")
        .args(&[
            "-n", &num_processes.to_string(),
            "--allow-run-as-root",
            benchmark_name,
            imb_test,
        ])
        .output()
        .ok()?;
    
    let mut result = MpiTestResult {
        test_type: test_type.to_string(),
        num_processes,
        size_bytes: 0,
        iterations: 0,
        success: output.status.success(),
        latency_us: None,
        bandwidth_mbps: None,
        min_latency_us: None,
        max_latency_us: None,
        avg_latency_us: None,
        error: None,
        raw_output: None,
    };
    
    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        result.raw_output = Some(output_str.to_string());
        parse_imb_output(&output_str, &mut result);
    } else {
        result.error = Some(String::from_utf8_lossy(&output.stderr).to_string());
    }
    
    Some(result)
}

/// Run a custom simple MPI test
fn run_custom_mpi_test(
    test_type: &str,
    num_processes: u32,
    size_bytes: u64,
    iterations: u32,
) -> Result<MpiTestResult, Box<dyn std::error::Error>> {
    let mut result = MpiTestResult {
        test_type: test_type.to_string(),
        num_processes,
        size_bytes,
        iterations,
        success: false,
        latency_us: None,
        bandwidth_mbps: None,
        min_latency_us: None,
        max_latency_us: None,
        avg_latency_us: None,
        error: Some(format!(
            "No MPI benchmarks found. To run {} test, please install:\n\
             - OSU Micro-Benchmarks: https://mvapich.cse.ohio-state.edu/benchmarks/\n\
             - Intel MPI Benchmarks: https://www.intel.com/content/www/us/en/developer/articles/technical/intel-mpi-benchmarks.html\n\
             \nInstallation examples:\n\
             # Ubuntu/Debian\n\
             sudo apt-get install libopenmpi-dev openmpi-bin\n\
             # Download and build OSU benchmarks\n\
             wget http://mvapich.cse.ohio-state.edu/download/mvapich/osu-micro-benchmarks-7.3.tar.gz\n\
             tar xf osu-micro-benchmarks-7.3.tar.gz\n\
             cd osu-micro-benchmarks-7.3\n\
             ./configure CC=mpicc CXX=mpicxx\n\
             make && sudo make install",
            test_type
        )),
        raw_output: None,
    };
    
    Ok(result)
}

/// Parse size string to bytes
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

/// Parse OSU benchmark output
fn parse_osu_output(output: &str, result: &mut MpiTestResult, target_size: u64) {
    // OSU output format:
    // # OSU MPI Latency Test v7.3
    // # Size          Latency (us)
    // 0               0.25
    // 1               0.26
    // ...
    
    let mut found_target = false;
    
    for line in output.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let (Ok(size), Ok(value)) = (parts[0].parse::<u64>(), parts[1].parse::<f64>()) {
                // For latency tests
                if size == target_size || !found_target {
                    result.latency_us = Some(value);
                    result.avg_latency_us = Some(value);
                    found_target = size == target_size;
                }
                
                // For bandwidth tests, second column might be bandwidth
                if parts.len() >= 2 && result.test_type.contains("bandwidth") {
                    if let Ok(bw) = parts[1].parse::<f64>() {
                        result.bandwidth_mbps = Some(bw);
                    }
                }
            }
        }
    }
}

/// Parse Intel MPI Benchmarks output
fn parse_imb_output(output: &str, result: &mut MpiTestResult) {
    // IMB output format varies, try to extract latency and bandwidth
    for line in output.lines() {
        if line.contains("Mbytes/sec") || line.contains("MB/s") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if let Ok(bw) = part.parse::<f64>() {
                    if bw > 0.0 && bw < 1000000.0 {
                        result.bandwidth_mbps = Some(bw);
                        break;
                    }
                }
            }
        }
        
        if line.contains("usec") || line.contains("μs") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for part in &parts {
                if let Ok(latency) = part.parse::<f64>() {
                    if latency > 0.0 && latency < 1000000.0 {
                        result.latency_us = Some(latency);
                        result.avg_latency_us = Some(latency);
                        break;
                    }
                }
            }
        }
    }
}

/// Parse MPI version information
fn parse_mpi_version(output: &str) -> (Option<String>, Option<String>) {
    let mut implementation = None;
    let mut version = None;
    
    for line in output.lines() {
        let lower = line.to_lowercase();
        
        // Detect implementation
        if lower.contains("open mpi") || lower.contains("openmpi") {
            implementation = Some("Open MPI".to_string());
            // Try to extract version from the same line
            if let Some(v) = extract_version(line) {
                version = Some(v);
            }
        } else if lower.contains("mpich") {
            implementation = Some("MPICH".to_string());
            if let Some(v) = extract_version(line) {
                version = Some(v);
            }
        } else if lower.contains("intel") && lower.contains("mpi") {
            implementation = Some("Intel MPI".to_string());
            if let Some(v) = extract_version(line) {
                version = Some(v);
            }
        } else if lower.contains("mvapich") {
            implementation = Some("MVAPICH".to_string());
            if let Some(v) = extract_version(line) {
                version = Some(v);
            }
        }
        
        // If we haven't found version yet, try generic version patterns
        if version.is_none() {
            if let Some(v) = extract_version(line) {
                version = Some(v);
            }
        }
    }
    
    (implementation, version)
}

/// Extract version number from a string
fn extract_version(s: &str) -> Option<String> {
    // Look for patterns like "version 4.1.5" or "v4.1.5" or just "4.1.5"
    let parts: Vec<&str> = s.split_whitespace().collect();
    
    for (i, part) in parts.iter().enumerate() {
        if part.to_lowercase().contains("version") && i + 1 < parts.len() {
            let next = parts[i + 1];
            if is_version_like(next) {
                return Some(next.trim_start_matches('v').to_string());
            }
        }
        
        if part.starts_with('v') || part.starts_with('V') {
            let version_part = &part[1..];
            if is_version_like(version_part) {
                return Some(version_part.to_string());
            }
        }
        
        if is_version_like(part) {
            return Some(part.to_string());
        }
    }
    
    None
}

/// Check if a string looks like a version number
fn is_version_like(s: &str) -> bool {
    let dot_count = s.chars().filter(|c| *c == '.').count();
    if dot_count < 1 || dot_count > 3 {
        return false;
    }
    
    let parts: Vec<&str> = s.split('.').collect();
    parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}
