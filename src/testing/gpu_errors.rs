use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
use crate::hardware::types::{GpuErrorInfo, GpuHealthInfo};
use serde::Serialize;

/// Collect GPU errors and health information using NVML
pub fn collect_gpu_errors() -> Result<Vec<GpuErrorInfo>, Box<dyn std::error::Error>> {
    let nvml = Nvml::init()?;
    let device_count = nvml.device_count()?;
    
    let mut errors = Vec::new();
    
    for i in 0..device_count {
        let device = nvml.device_by_index(i)?;
        
        // Get device name and UUID
        let name = device.name().unwrap_or_else(|_| format!("GPU {}", i));
        let uuid = device.uuid().ok();
        
        // Collect various error types
        let mut error_info = GpuErrorInfo {
            device_index: i,
            device_name: name.clone(),
            device_uuid: uuid.clone(),
            ecc_errors: None,
            retired_pages: None,
            xid_errors: None,
            thermal_violations: None,
            power_violations: None,
            has_errors: false,
        };
        
        // Check for ECC errors (if supported)
        if let Ok(ecc_mode) = device.is_ecc_enabled() {
            if ecc_mode.currently_enabled {
                let ecc_errors = collect_ecc_errors(&device);
                if ecc_errors.has_errors {
                    error_info.has_errors = true;
                }
                error_info.ecc_errors = Some(ecc_errors);
            }
        }
        
        // Check for retired pages (memory errors)
        use nvml_wrapper::enum_wrappers::device::RetirementCause;
        
        let mut total_retired = 0u32;
        if let Ok(pages) = device.retired_pages(RetirementCause::MultipleSingleBitEccErrors) {
            total_retired += pages.len() as u32;
        }
        if let Ok(pages) = device.retired_pages(RetirementCause::DoubleBitEccError) {
            total_retired += pages.len() as u32;
        }
        
        if total_retired > 0 {
            error_info.has_errors = true;
            error_info.retired_pages = Some(total_retired);
        }
        
        // Check for thermal violations
        if let Ok(_violations) = device.total_energy_consumption() {
            // Note: This is energy consumption, not violations
            // For actual thermal violations, we'd need to check temperature thresholds
            if let Ok(temp) = device.temperature(TemperatureSensor::Gpu) {
                if let Ok(threshold) = device.temperature_threshold(nvml_wrapper::enum_wrappers::device::TemperatureThreshold::Slowdown) {
                    if temp >= threshold as u32 {
                        error_info.thermal_violations = Some(format!("Temperature {}°C exceeds slowdown threshold {}°C", temp, threshold));
                        error_info.has_errors = true;
                    }
                }
            }
        }
        
        errors.push(error_info);
    }
    
    Ok(errors)
}

/// Collect detailed ECC error information
fn collect_ecc_errors(device: &nvml_wrapper::Device) -> EccErrorCounts {
    let mut ecc_errors = EccErrorCounts {
        volatile_single_bit: 0,
        volatile_double_bit: 0,
        aggregate_single_bit: 0,
        aggregate_double_bit: 0,
        has_errors: false,
    };
    
    use nvml_wrapper::enum_wrappers::device::{EccCounter, MemoryError};
    
    // Volatile (current session) errors
    if let Ok(sbe) = device.total_ecc_errors(MemoryError::Corrected, EccCounter::Volatile) {
        ecc_errors.volatile_single_bit = sbe;
        if sbe > 0 {
            ecc_errors.has_errors = true;
        }
    }
    
    if let Ok(dbe) = device.total_ecc_errors(MemoryError::Uncorrected, EccCounter::Volatile) {
        ecc_errors.volatile_double_bit = dbe;
        if dbe > 0 {
            ecc_errors.has_errors = true;
        }
    }
    
    // Aggregate (lifetime) errors
    if let Ok(sbe) = device.total_ecc_errors(MemoryError::Corrected, EccCounter::Aggregate) {
        ecc_errors.aggregate_single_bit = sbe;
    }
    
    if let Ok(dbe) = device.total_ecc_errors(MemoryError::Uncorrected, EccCounter::Aggregate) {
        ecc_errors.aggregate_double_bit = dbe;
    }
    
    ecc_errors
}

/// Collect comprehensive GPU health information
pub fn collect_gpu_health() -> Result<Vec<GpuHealthInfo>, Box<dyn std::error::Error>> {
    let nvml = Nvml::init()?;
    let device_count = nvml.device_count()?;
    
    let mut health_info = Vec::new();
    
    for i in 0..device_count {
        let device = nvml.device_by_index(i)?;
        
        let name = device.name().unwrap_or_else(|_| format!("GPU {}", i));
        let uuid = device.uuid().ok();
        
        let mut info = GpuHealthInfo {
            device_index: i,
            device_name: name,
            device_uuid: uuid,
            temperature_celsius: None,
            power_usage_watts: None,
            power_limit_watts: None,
            fan_speed_percent: None,
            utilization_gpu_percent: None,
            utilization_memory_percent: None,
            memory_used_mb: None,
            memory_total_mb: None,
            clock_graphics_mhz: None,
            clock_memory_mhz: None,
            throttle_reasons: Vec::new(),
            performance_state: None,
        };
        
        // Temperature
        if let Ok(temp) = device.temperature(TemperatureSensor::Gpu) {
            info.temperature_celsius = Some(temp);
        }
        
        // Power usage
        if let Ok(power) = device.power_usage() {
            info.power_usage_watts = Some((power / 1000) as u32); // Convert mW to W
        }
        
        if let Ok(power_limit) = device.power_management_limit() {
            info.power_limit_watts = Some((power_limit / 1000) as u32);
        }
        
        // Fan speed
        if let Ok(fan_speed) = device.fan_speed(0) {
            info.fan_speed_percent = Some(fan_speed);
        }
        
        // Utilization
        if let Ok(utilization) = device.utilization_rates() {
            info.utilization_gpu_percent = Some(utilization.gpu);
            info.utilization_memory_percent = Some(utilization.memory);
        }
        
        // Memory
        if let Ok(memory_info) = device.memory_info() {
            info.memory_used_mb = Some((memory_info.used / (1024 * 1024)) as u32);
            info.memory_total_mb = Some((memory_info.total / (1024 * 1024)) as u32);
        }
        
        // Clock speeds
        if let Ok(graphics_clock) = device.clock_info(Clock::Graphics) {
            info.clock_graphics_mhz = Some(graphics_clock);
        }
        
        if let Ok(memory_clock) = device.clock_info(Clock::Memory) {
            info.clock_memory_mhz = Some(memory_clock);
        }
        
        // Throttle reasons
        if let Ok(throttle_reasons) = device.current_throttle_reasons() {
            use nvml_wrapper::bitmasks::device::ThrottleReasons;
            
            if throttle_reasons.contains(ThrottleReasons::GPU_IDLE) {
                info.throttle_reasons.push("GPU Idle".to_string());
            }
            if throttle_reasons.contains(ThrottleReasons::SW_THERMAL_SLOWDOWN) {
                info.throttle_reasons.push("Software Thermal Slowdown".to_string());
            }
            if throttle_reasons.contains(ThrottleReasons::HW_THERMAL_SLOWDOWN) {
                info.throttle_reasons.push("Hardware Thermal Slowdown".to_string());
            }
            if throttle_reasons.contains(ThrottleReasons::HW_POWER_BRAKE_SLOWDOWN) {
                info.throttle_reasons.push("Hardware Power Brake Slowdown".to_string());
            }
            if throttle_reasons.contains(ThrottleReasons::SW_POWER_CAP) {
                info.throttle_reasons.push("Software Power Cap".to_string());
            }
        }
        
        // Performance state
        if let Ok(pstate) = device.performance_state() {
            info.performance_state = Some(format!("P{}", pstate as u32));
        }
        
        health_info.push(info);
    }
    
    Ok(health_info)
}

#[derive(Debug, Serialize)]
pub struct EccErrorCounts {
    pub volatile_single_bit: u64,
    pub volatile_double_bit: u64,
    pub aggregate_single_bit: u64,
    pub aggregate_double_bit: u64,
    pub has_errors: bool,
}
