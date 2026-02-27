use crate::hardware::types::{DcgmInfo, DcgmDiagResult, DcgmGpuDiagResult, DcgmHealthCheck, DcgmIncident};
use std::process::Command;

/// Get DCGM installation information and version
pub fn collect_dcgm_info() -> DcgmInfo {
    let mut info = DcgmInfo {
        dcgm_version: None,
        dcgm_available: false,
        dcgmi_available: false,
        num_gpus: 0,
        driver_version: None,
        cuda_driver_version: None,
        error: None,
    };
    
    // Check if dcgmi is available
    if let Ok(output) = Command::new("which").arg("dcgmi").output() {
        if !output.status.success() {
            info.error = Some("dcgmi not found. Please install DCGM (Data Center GPU Manager).".to_string());
            return info;
        }
        info.dcgmi_available = true;
    }
    
    // Get DCGM version
    if let Ok(output) = Command::new("dcgmi").arg("--version").output() {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            if let Some(version) = parse_dcgm_version(&output_str) {
                info.dcgm_version = Some(version);
                info.dcgm_available = true;
            }
        }
    }
    
    // Get GPU count and driver version using dcgmi discovery
    if info.dcgmi_available {
        if let Ok(output) = Command::new("dcgmi").arg("discovery").arg("-l").output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                info.num_gpus = count_gpus_in_discovery(&output_str);
            }
        }
        
        // Get driver version
        if let Ok(output) = Command::new("nvidia-smi").arg("--query-gpu=driver_version").arg("--format=csv,noheader").output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if let Some(version) = output_str.lines().next() {
                    info.driver_version = Some(version.trim().to_string());
                }
            }
        }
        
        // Get CUDA driver version
        if let Ok(output) = Command::new("nvidia-smi").output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if let Some(version) = parse_cuda_version(&output_str) {
                    info.cuda_driver_version = Some(version);
                }
            }
        }
    }
    
    info
}

/// Parse DCGM version from version output
fn parse_dcgm_version(output: &str) -> Option<String> {
    // Look for lines like "DCGM version: 3.1.7" or "Version: 3.1.7"
    for line in output.lines() {
        if line.contains("version") || line.contains("Version") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                let version = parts[1].trim();
                if !version.is_empty() {
                    return Some(version.to_string());
                }
            }
        }
    }
    
    // Fallback: look for version patterns
    for line in output.lines() {
        if let Some(start) = line.find(char::is_numeric) {
            let version_part = &line[start..];
            let version: String = version_part.chars()
                .take_while(|c| c.is_numeric() || *c == '.')
                .collect();
            if !version.is_empty() {
                return Some(version);
            }
        }
    }
    
    None
}

/// Count GPUs from dcgmi discovery output
fn count_gpus_in_discovery(output: &str) -> u32 {
    let mut count = 0;
    for line in output.lines() {
        if line.contains("GPU") && line.contains("->") {
            count += 1;
        }
    }
    count
}

/// Parse CUDA version from nvidia-smi output
fn parse_cuda_version(output: &str) -> Option<String> {
    for line in output.lines() {
        if line.contains("CUDA Version") {
            if let Some(start) = line.find("CUDA Version:") {
                let version_part = &line[start + 13..];
                let version: String = version_part.trim()
                    .chars()
                    .take_while(|c| c.is_numeric() || *c == '.')
                    .collect();
                if !version.is_empty() {
                    return Some(version);
                }
            }
        }
    }
    None
}

/// Run DCGM diagnostic tests
/// 
/// Note: This command will create NVVS (NVIDIA Validation Suite) log files
/// in the current directory as DCGM uses NVVS as its underlying diagnostic engine.
pub fn run_dcgm_diag(level: u32, gpu_ids: Option<Vec<u32>>) 
    -> Result<DcgmDiagResult, Box<dyn std::error::Error>> {
    
    let mut result = DcgmDiagResult {
        test_name: format!("DCGM Diagnostics Level {}", level),
        success: false,
        gpu_results: Vec::new(),
        overall_result: "Unknown".to_string(),
        time_seconds: None,
        error: None,
        raw_output: None,
    };
    
    // Check if dcgmi is available
    if !Command::new("which")
        .arg("dcgmi")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        result.error = Some("dcgmi not found. Please install DCGM.".to_string());
        return Ok(result);
    }
    
    // Validate diagnostic level (1-4)
    if level < 1 || level > 4 {
        result.error = Some("Diagnostic level must be between 1 and 4.".to_string());
        return Ok(result);
    }
    
    // Build command
    let mut cmd = Command::new("dcgmi");
    cmd.arg("diag");
    cmd.arg("-r");
    cmd.arg(level.to_string());
    
    // Specify GPUs if provided
    if let Some(gpus) = &gpu_ids {
        if !gpus.is_empty() {
            let gpu_str = gpus.iter()
                .map(|g| g.to_string())
                .collect::<Vec<_>>()
                .join(",");
            cmd.arg("-i");
            cmd.arg(gpu_str);
        }
    }
    
    // Run the diagnostic
    let start_time = std::time::Instant::now();
    let output = cmd.output()?;
    let elapsed = start_time.elapsed().as_secs_f64();
    
    result.time_seconds = Some(elapsed);
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let error_str = String::from_utf8_lossy(&output.stderr);
    result.raw_output = Some(format!("{}\n{}", output_str, error_str));
    
    // Check for errors in stdout (DCGM often reports errors there)
    let has_stdout_error = output_str.to_lowercase().contains("error") 
        || output_str.to_lowercase().contains("unsupported")
        || output_str.to_lowercase().contains("failed");
    
    // Parse results
    if output.status.success() && !has_stdout_error {
        result.success = true;
        parse_diag_results(&output_str, &mut result);
    } else {
        // Capture error from either stdout or stderr
        let error_msg = if !error_str.trim().is_empty() {
            error_str.to_string()
        } else if has_stdout_error {
            // Extract error lines from stdout
            output_str.lines()
                .filter(|line| {
                    let lower = line.to_lowercase();
                    lower.contains("error") || lower.contains("unsupported") || lower.contains("failed")
                })
                .collect::<Vec<_>>()
                .join("; ")
        } else {
            "Diagnostic failed with unknown error".to_string()
        };
        
        result.error = Some(format!("Diagnostic failed: {}", error_msg.trim()));
        result.overall_result = "Fail".to_string();
    }
    
    Ok(result)
}

/// Parse DCGM diagnostic results from output
fn parse_diag_results(output: &str, result: &mut DcgmDiagResult) {
    let mut current_gpu_index = None;
    let mut current_gpu_name = None;
    
    for line in output.lines() {
        let trimmed = line.trim();
        
        // Look for overall result
        if trimmed.contains("Overall Result:") || trimmed.contains("Result:") {
            if let Some(result_part) = trimmed.split(':').nth(1) {
                let status = result_part.trim();
                if status.to_lowercase().contains("pass") {
                    result.overall_result = "Pass".to_string();
                } else if status.to_lowercase().contains("fail") {
                    result.overall_result = "Fail".to_string();
                } else if status.to_lowercase().contains("warn") {
                    result.overall_result = "Warning".to_string();
                } else {
                    result.overall_result = status.to_string();
                }
            }
        }
        
        // Parse GPU results
        if trimmed.starts_with("GPU") {
            // Try to extract GPU index
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(idx) = parts[1].trim_matches(':').parse::<u32>() {
                    current_gpu_index = Some(idx);
                }
            }
        }
        
        // Look for pass/fail indicators per GPU
        if trimmed.contains("PASS") || trimmed.contains("FAIL") || trimmed.contains("SKIP") {
            if let Some(idx) = current_gpu_index {
                let gpu_result = if trimmed.contains("PASS") {
                    "Pass"
                } else if trimmed.contains("FAIL") {
                    "Fail"
                } else {
                    "Skip"
                };
                
                result.gpu_results.push(DcgmGpuDiagResult {
                    device_index: idx,
                    device_name: current_gpu_name.clone(),
                    result: gpu_result.to_string(),
                    info: Some(trimmed.to_string()),
                });
                
                current_gpu_index = None;
                current_gpu_name = None;
            }
        }
    }
    
    // If no overall result was found but all GPU results passed
    if result.overall_result == "Unknown" && !result.gpu_results.is_empty() {
        let all_pass = result.gpu_results.iter().all(|r| r.result == "Pass");
        result.overall_result = if all_pass { "Pass".to_string() } else { "Fail".to_string() };
    }
}

/// Run DCGM health check
pub fn run_dcgm_health_check() -> Result<Vec<DcgmHealthCheck>, Box<dyn std::error::Error>> {
    // Check if dcgmi is available
    if !Command::new("which")
        .arg("dcgmi")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Err("dcgmi not found. Please install DCGM.".into());
    }
    
    // Run dcgmi health check
    let output = Command::new("dcgmi")
        .arg("health")
        .arg("-c")
        .output()?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    
    if !output.status.success() {
        let error_str = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Health check failed: {}", error_str).into());
    }
    
    // Parse health check results
    let health_results = parse_health_check(&output_str);
    
    Ok(health_results)
}

/// Parse DCGM health check output
fn parse_health_check(output: &str) -> Vec<DcgmHealthCheck> {
    let mut results = Vec::new();
    let mut current_gpu_index = None;
    let mut current_gpu_name = None;
    let mut current_incidents = Vec::new();
    
    for line in output.lines() {
        let trimmed = line.trim();
        
        // Parse GPU identifier
        if trimmed.starts_with("GPU") {
            // Save previous GPU if exists
            if let Some(idx) = current_gpu_index {
                let health_status = if current_incidents.is_empty() {
                    "Healthy"
                } else {
                    let has_critical = current_incidents.iter().any(|i: &DcgmIncident| i.severity == "Critical" || i.severity == "Error");
                    if has_critical { "Failure" } else { "Warning" }
                };
                
                results.push(DcgmHealthCheck {
                    device_index: idx,
                    device_name: current_gpu_name.clone(),
                    health_status: health_status.to_string(),
                    incidents: current_incidents.clone(),
                });
                
                current_incidents.clear();
            }
            
            // Parse new GPU index
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(idx) = parts[1].trim_matches(':').parse::<u32>() {
                    current_gpu_index = Some(idx);
                    current_gpu_name = None;
                }
            }
        }
        
        // Parse incidents (errors, warnings, etc.)
        if trimmed.contains("Warning:") || trimmed.contains("Error:") || trimmed.contains("Critical:") {
            let severity = if trimmed.contains("Critical:") {
                "Critical"
            } else if trimmed.contains("Error:") {
                "Error"
            } else {
                "Warning"
            };
            
            let message = trimmed.to_string();
            
            current_incidents.push(DcgmIncident {
                incident_type: "Health Check".to_string(),
                severity: severity.to_string(),
                message,
                timestamp: None,
            });
        }
        
        // Check for status indicators
        if trimmed.contains("Healthy") || trimmed.contains("OK") {
            // GPU is healthy, no action needed
        }
    }
    
    // Add the last GPU if exists
    if let Some(idx) = current_gpu_index {
        let health_status = if current_incidents.is_empty() {
            "Healthy"
        } else {
            let has_critical = current_incidents.iter().any(|i| i.severity == "Critical" || i.severity == "Error");
            if has_critical { "Failure" } else { "Warning" }
        };
        
        results.push(DcgmHealthCheck {
            device_index: idx,
            device_name: current_gpu_name,
            health_status: health_status.to_string(),
            incidents: current_incidents,
        });
    }
    
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_dcgm_version() {
        let output = "DCGM version: 3.1.7";
        let version = parse_dcgm_version(output);
        assert_eq!(version, Some("3.1.7".to_string()));
    }
    
    #[test]
    fn test_count_gpus() {
        let output = "GPU 0: Tesla V100 -> Healthy\nGPU 1: Tesla V100 -> Healthy";
        let count = count_gpus_in_discovery(output);
        assert_eq!(count, 2);
    }
}
