use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct Inventory {
    pub agent_version: String,
    pub node: NodeInfo,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub network: NetworkInfo,
    pub gpus: Vec<GpuInfo>,
    pub power_supplies: Vec<PowerSupplyInfo>,
}

#[derive(Debug, Serialize)]
pub struct NodeInfo {
    pub hostname: String,
    pub architecture: String,
    pub product_name: Option<String>,
    pub manufacturer: Option<String>,
    pub serial_number: Option<String>,
    pub chassis_manufacturer: Option<String>,
    pub chassis_serial_number: Option<String>,
    pub motherboard: Option<MotherboardInfo>,
    pub bios: Option<BiosInfo>,
    pub bmc: Option<BmcInfo>,
}

#[derive(Debug, Serialize)]
pub struct MotherboardInfo {
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
    pub version: Option<String>,
    pub serial_number: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BiosInfo {
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub release_date: Option<String>,
}


#[derive(Debug, Serialize)]
pub struct BmcInfo {
    pub ip_address: Option<String>,
    pub mac_address: Option<String>,
    pub firmware_version: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CpuInfo {
    pub sockets: Option<u32>,
    pub cores: Option<u32>,
    pub threads: Option<u32>,
    pub cpus: Vec<CpuSocket>,
}

#[derive(Debug, Serialize)]
pub struct CpuSocket {
    pub socket: u32,
    pub manufacturer: Option<String>,
    pub model_name: Option<String>,
    pub num_cores: Option<u32>,
    pub num_threads: Option<u32>,
    pub capacity_mhz: Option<u32>,
    pub slot: Option<String>,
    pub l1_cache_kb: Option<u32>,
    pub l2_cache_kb: Option<u32>,
    pub l3_cache_kb: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct MemoryInfo {
    pub total_bytes: Option<u64>,
    pub dimms: Vec<DimmInfo>,
}

#[derive(Debug, Serialize)]
pub struct DimmInfo {
    pub slot: Option<String>,
    pub size_bytes: Option<u64>,
    pub mem_type: Option<String>,
    pub speed_mt_s: Option<u32>,
    pub manufacturer: Option<String>,
    pub serial_number: Option<String>,
    pub part_number: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub dev_path: String,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub size_bytes: Option<u64>,
    pub rotational: Option<bool>,
    pub bus_type: Option<String>, // "nvme", "scsi", "virtio", etc.
    pub firmware_version: Option<String>,
    pub smart: Option<SmartInfo>,
}

#[derive(Debug, Serialize)]
pub struct SmartInfo {
    pub health: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NetworkInfo {
    pub interfaces: Vec<NetInterface>,
    pub routes: Vec<RouteInfo>,
}

#[derive(Debug, Serialize)]
pub struct NetInterface {
    pub name: String,
    pub mac_address: Option<String>,
    pub mtu: Option<u32>,
    pub speed_mbps: Option<u32>,
    pub driver: Option<String>,
    pub firmware_version: Option<String>,
    pub vendor_name: Option<String>,
    pub device_name: Option<String>,
    pub pci_address: Option<String>,
    pub addresses: Vec<IpAddress>,
    
    // Bond/Team configuration
    pub is_primary: bool,
    pub bond_group: Option<String>,
    pub bond_master: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct IpAddress {
    pub family: String, // "IPv4" or "IPv6"
    pub address: String,
    pub prefix: u8,
}

#[derive(Debug, Serialize)]
pub struct RouteInfo {
    pub dst: String,     // CIDR
    pub gateway: String, // IP
    pub iface: String,
}

#[derive(Debug, Serialize)]
pub struct GpuInfo {
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub pci_address: Option<String>,
    pub vram_mb: Option<u32>,
    pub driver_version: Option<String>,
    pub uuid: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GpuErrorInfo {
    pub device_index: u32,
    pub device_name: String,
    pub device_uuid: Option<String>,
    pub ecc_errors: Option<crate::testing::gpu_errors::EccErrorCounts>,
    pub retired_pages: Option<u32>,
    pub xid_errors: Option<String>,
    pub thermal_violations: Option<String>,
    pub power_violations: Option<String>,
    pub has_errors: bool,
}

#[derive(Debug, Serialize)]
pub struct GpuHealthInfo {
    pub device_index: u32,
    pub device_name: String,
    pub device_uuid: Option<String>,
    pub temperature_celsius: Option<u32>,
    pub power_usage_watts: Option<u32>,
    pub power_limit_watts: Option<u32>,
    pub fan_speed_percent: Option<u32>,
    pub utilization_gpu_percent: Option<u32>,
    pub utilization_memory_percent: Option<u32>,
    pub memory_used_mb: Option<u32>,
    pub memory_total_mb: Option<u32>,
    pub clock_graphics_mhz: Option<u32>,
    pub clock_memory_mhz: Option<u32>,
    pub throttle_reasons: Vec<String>,
    pub performance_state: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NcclInfo {
    pub nccl_version: Option<String>,
    pub cuda_version: Option<String>,
    pub num_gpus: u32,
    pub nccl_available: bool,
    pub nccl_tests_available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NcclTestResult {
    pub test_type: String,
    pub size_bytes: u64,
    pub iterations: u32,
    pub num_gpus: u32,
    pub success: bool,
    pub time_us: Option<f64>,
    pub bandwidth_gbps: Option<f64>,
    pub bus_bandwidth_gbps: Option<f64>,
    pub error: Option<String>,
    pub gpu_results: Vec<NcclGpuResult>,
}

#[derive(Debug, Serialize)]
pub struct NcclGpuResult {
    pub device_index: u32,
    pub device_name: String,
    pub in_place: bool,
    pub out_of_place: bool,
}

#[derive(Debug, Serialize)]
pub struct MpiInfo {
    pub mpi_version: Option<String>,
    pub mpi_implementation: Option<String>,
    pub mpi_available: bool,
    pub mpirun_available: bool,
    pub num_cpus: u32,
    pub mpi_benchmark_available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MpiTestResult {
    pub test_type: String,
    pub num_processes: u32,
    pub size_bytes: u64,
    pub iterations: u32,
    pub success: bool,
    pub latency_us: Option<f64>,
    pub bandwidth_mbps: Option<f64>,
    pub min_latency_us: Option<f64>,
    pub max_latency_us: Option<f64>,
    pub avg_latency_us: Option<f64>,
    pub error: Option<String>,
    pub raw_output: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HashcatInfo {
    pub hashcat_version: Option<String>,
    pub hashcat_available: bool,
    pub opencl_available: bool,
    pub cuda_available: bool,
    pub num_devices: u32,
    pub devices: Vec<HashcatDevice>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HashcatDevice {
    pub device_id: u32,
    pub device_name: String,
    pub device_type: String, // "GPU", "CPU"
    pub opencl_version: Option<String>,
    pub cuda_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HashcatTestResult {
    pub test_type: String, // "benchmark", "dictionary", "brute-force"
    pub hash_type: Option<String>, // e.g., "MD5", "SHA256", "bcrypt"
    pub device_ids: Vec<u32>,
    pub success: bool,
    pub hash_speed: Option<f64>, // Hashes per second
    pub time_seconds: Option<f64>,
    pub recovered: Option<u32>,
    pub total: Option<u32>,
    pub error: Option<String>,
    pub raw_output: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DcgmInfo {
    pub dcgm_version: Option<String>,
    pub dcgm_available: bool,
    pub dcgmi_available: bool,
    pub num_gpus: u32,
    pub driver_version: Option<String>,
    pub cuda_driver_version: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DcgmDiagResult {
    pub test_name: String,
    pub success: bool,
    pub gpu_results: Vec<DcgmGpuDiagResult>,
    pub overall_result: String, // "Pass", "Fail", "Warning"
    pub time_seconds: Option<f64>,
    pub error: Option<String>,
    pub raw_output: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DcgmGpuDiagResult {
    pub device_index: u32,
    pub device_name: Option<String>,
    pub result: String, // "Pass", "Fail", "Skip", "Warning"
    pub info: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DcgmHealthCheck {
    pub device_index: u32,
    pub device_name: Option<String>,
    pub health_status: String, // "Healthy", "Warning", "Failure"
    pub incidents: Vec<DcgmIncident>,
}

#[derive(Debug, Serialize, Clone)]
pub struct DcgmIncident {
    pub incident_type: String,
    pub severity: String, // "Info", "Warning", "Error", "Critical"
    pub message: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Timestamps {
    pub collected_at: String,
    pub agent_version: String,
}

#[derive(Debug, Serialize)]
pub struct PowerSupplyInfo {
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub part_number: Option<String>,
    pub max_power_watts: Option<u32>,
    pub efficiency_rating: Option<String>, // "80 Plus Gold", "80 Plus Platinum", etc.
    pub status: Option<String>, // "OK", "Critical", "Non-critical", etc.
    pub input_voltage: Option<f32>,
    pub input_current: Option<f32>,
    pub output_voltage: Option<f32>,
    pub output_current: Option<f32>,
    pub temperature_c: Option<i32>,
    pub fan_speed_rpm: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct RawBlobs {
    pub lshw: Option<serde_json::Value>,
    pub lsblk: Option<serde_json::Value>,
    pub lspci: Option<serde_json::Value>,
    pub dmidecode: Option<serde_json::Value>,
    pub extra: HashMap<String, serde_json::Value>,
}
