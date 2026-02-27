use crate::cli::VmCommands;
use crate::output::output_data;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::process::Command;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
struct VmInfo {
    name: String,
    state: String,
    id: Option<String>,
    uuid: Option<String>,
}

pub fn handle_vm_command(cmd: &VmCommands) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        VmCommands::List { hypervisor, format } => {
            list_vms(hypervisor, format)?;
        }
        
        VmCommands::Start { name, hypervisor } => {
            start_vm(name, hypervisor)?;
        }
        
        VmCommands::Stop { name, hypervisor, force } => {
            stop_vm(name, hypervisor, *force)?;
        }
        
        VmCommands::Create { 
            name, 
            hypervisor, 
            vcpus, 
            memory, 
            disk, 
            os_variant, 
            iso, 
            network 
        } => {
            create_vm(name, hypervisor, *vcpus, *memory, *disk, os_variant.as_deref(), iso.as_deref(), network)?;
        }
        
        VmCommands::Delete { name, hypervisor, remove_storage, yes } => {
            delete_vm(name, hypervisor, *remove_storage, *yes)?;
        }
        
        VmCommands::Status { name, hypervisor, format } => {
            vm_status(name, hypervisor, format)?;
        }
        
        VmCommands::Reboot { name, hypervisor, force } => {
            reboot_vm(name, hypervisor, *force)?;
        }
        
        VmCommands::PostInventory { url, hypervisor } => {
            println!("Collecting VM inventory...");
            let inventory = collect_vm_inventory(hypervisor)?;
            
            println!("Host MAC address: {}", inventory.host_mac_address);
            
            let api_url = format!("{}/api/v1/vms/inventory", url.trim_end_matches('/'));
            println!("Posting VM inventory to: {}", api_url);
            
            let client = reqwest::blocking::Client::new();
            let response = client
                .post(&api_url)
                .json(&inventory)
                .send()?;
            
            if response.status().is_success() {
                let result: serde_json::Value = response.json()?;
                println!("✓ Success!");
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                let status = response.status();
                let error_text = response.text()?;
                eprintln!("✗ Error: HTTP {}", status);
                eprintln!("{}", error_text);
                return Err(format!("Failed to post VM inventory: HTTP {}", status).into());
            }
        }
    }
    Ok(())
}

fn list_vms(hypervisor: &str, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    match hypervisor {
        "kvm" | "qemu" => {
            println!("Listing VMs via virsh...");
            let output = Command::new("virsh")
                .args(&["list", "--all"])
                .output()?;
            
            if !output.status.success() {
                return Err(format!("virsh command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
            }
            
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            if format == "pretty" {
                println!("{}", stdout);
            } else {
                // Parse and format as JSON/YAML
                let vms = parse_virsh_list(&stdout)?;
                output_data(&vms, format)?;
            }
        }
        
        "virtualbox" => {
            println!("Listing VMs via VBoxManage...");
            let output = Command::new("VBoxManage")
                .args(&["list", "vms", "--long"])
                .output()?;
            
            if !output.status.success() {
                return Err(format!("VBoxManage command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
            }
            
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }
        
        _ => {
            return Err(format!("Unsupported hypervisor: {}", hypervisor).into());
        }
    }
    
    Ok(())
}

fn start_vm(name: &str, hypervisor: &str) -> Result<(), Box<dyn std::error::Error>> {
    match hypervisor {
        "kvm" | "qemu" => {
            println!("Starting VM '{}' via virsh...", name);
            let output = Command::new("virsh")
                .args(&["start", name])
                .output()?;
            
            if output.status.success() {
                println!("✓ VM '{}' started successfully", name);
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to start VM: {}", error).into());
            }
        }
        
        "virtualbox" => {
            println!("Starting VM '{}' via VBoxManage...", name);
            let output = Command::new("VBoxManage")
                .args(&["startvm", name, "--type", "headless"])
                .output()?;
            
            if output.status.success() {
                println!("✓ VM '{}' started successfully", name);
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to start VM: {}", error).into());
            }
        }
        
        _ => {
            return Err(format!("Unsupported hypervisor: {}", hypervisor).into());
        }
    }
    
    Ok(())
}

fn stop_vm(name: &str, hypervisor: &str, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    match hypervisor {
        "kvm" | "qemu" => {
            let action = if force { "destroy" } else { "shutdown" };
            println!("{} VM '{}' via virsh...", if force { "Forcing stop of" } else { "Shutting down" }, name);
            
            let output = Command::new("virsh")
                .args(&[action, name])
                .output()?;
            
            if output.status.success() {
                println!("✓ VM '{}' {} successfully", name, if force { "stopped" } else { "shutdown initiated" });
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to stop VM: {}", error).into());
            }
        }
        
        "virtualbox" => {
            let action_type = if force { "poweroff" } else { "acpipowerbutton" };
            println!("{} VM '{}' via VBoxManage...", if force { "Forcing stop of" } else { "Shutting down" }, name);
            
            let output = Command::new("VBoxManage")
                .args(&["controlvm", name, action_type])
                .output()?;
            
            if output.status.success() {
                println!("✓ VM '{}' {} successfully", name, if force { "stopped" } else { "shutdown initiated" });
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to stop VM: {}", error).into());
            }
        }
        
        _ => {
            return Err(format!("Unsupported hypervisor: {}", hypervisor).into());
        }
    }
    
    Ok(())
}

fn create_vm(
    name: &str,
    hypervisor: &str,
    vcpus: u32,
    memory: u32,
    disk: u32,
    os_variant: Option<&str>,
    iso: Option<&str>,
    network: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match hypervisor {
        "kvm" | "qemu" => {
            println!("Creating VM '{}' via virt-install...", name);
            
            let mut args = vec![
                "--name".to_string(),
                name.to_string(),
                "--vcpus".to_string(),
                vcpus.to_string(),
                "--memory".to_string(),
                memory.to_string(),
                "--disk".to_string(),
                format!("size={}", disk),
            ];
            
            // Add OS variant if provided
            if let Some(os) = os_variant {
                args.push("--os-variant".to_string());
                args.push(os.to_string());
            }
            
            // Add ISO if provided
            if let Some(iso_path) = iso {
                args.push("--cdrom".to_string());
                args.push(iso_path.to_string());
            } else {
                args.push("--pxe".to_string());
            }
            
            // Add network
            if network != "none" {
                args.push("--network".to_string());
                if network == "default" {
                    args.push("network=default".to_string());
                } else {
                    args.push(format!("bridge={}", network));
                }
            }
            
            // Graphics and console
            args.push("--graphics".to_string());
            args.push("vnc,listen=0.0.0.0".to_string());
            args.push("--noautoconsole".to_string());
            
            let output = Command::new("virt-install")
                .args(&args)
                .output()?;
            
            if output.status.success() {
                println!("✓ VM '{}' created successfully", name);
                println!("{}", String::from_utf8_lossy(&output.stdout));
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to create VM: {}", error).into());
            }
        }
        
        "virtualbox" => {
            println!("Creating VM '{}' via VBoxManage...", name);
            
            // Create the VM
            let output = Command::new("VBoxManage")
                .args(&["createvm", "--name", name, "--ostype", os_variant.unwrap_or("Linux_64"), "--register"])
                .output()?;
            
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to create VM: {}", error).into());
            }
            
            // Configure VM
            Command::new("VBoxManage")
                .args(&["modifyvm", name, "--cpus", &vcpus.to_string(), "--memory", &memory.to_string()])
                .output()?;
            
            // Create disk
            let disk_path = format!("/var/lib/virtualbox/{}.vdi", name);
            Command::new("VBoxManage")
                .args(&["createhd", "--filename", &disk_path, "--size", &(disk * 1024).to_string()])
                .output()?;
            
            // Attach disk
            Command::new("VBoxManage")
                .args(&["storagectl", name, "--name", "SATA", "--add", "sata", "--controller", "IntelAhci"])
                .output()?;
                
            Command::new("VBoxManage")
                .args(&["storageattach", name, "--storagectl", "SATA", "--port", "0", "--device", "0", "--type", "hdd", "--medium", &disk_path])
                .output()?;
            
            println!("✓ VM '{}' created successfully", name);
        }
        
        _ => {
            return Err(format!("Unsupported hypervisor: {}", hypervisor).into());
        }
    }
    
    Ok(())
}

fn delete_vm(name: &str, hypervisor: &str, remove_storage: bool, yes: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !yes {
        print!("Are you sure you want to delete VM '{}'? [y/N]: ", name);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }
    
    match hypervisor {
        "kvm" | "qemu" => {
            println!("Deleting VM '{}' via virsh...", name);
            
            // Stop VM if running
            let _ = Command::new("virsh")
                .args(&["destroy", name])
                .output();
            
            // Undefine with optional storage removal
            let mut args = vec!["undefine", name];
            if remove_storage {
                args.push("--remove-all-storage");
            }
            
            let output = Command::new("virsh")
                .args(&args)
                .output()?;
            
            if output.status.success() {
                println!("✓ VM '{}' deleted successfully", name);
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to delete VM: {}", error).into());
            }
        }
        
        "virtualbox" => {
            println!("Deleting VM '{}' via VBoxManage...", name);
            
            // Stop VM if running
            let _ = Command::new("VBoxManage")
                .args(&["controlvm", name, "poweroff"])
                .output();
            
            // Wait a moment
            std::thread::sleep(std::time::Duration::from_secs(1));
            
            // Unregister and delete
            let mut args = vec!["unregistervm", name];
            if remove_storage {
                args.push("--delete");
            }
            
            let output = Command::new("VBoxManage")
                .args(&args)
                .output()?;
            
            if output.status.success() {
                println!("✓ VM '{}' deleted successfully", name);
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to delete VM: {}", error).into());
            }
        }
        
        _ => {
            return Err(format!("Unsupported hypervisor: {}", hypervisor).into());
        }
    }
    
    Ok(())
}

fn vm_status(name: &str, hypervisor: &str, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    match hypervisor {
        "kvm" | "qemu" => {
            println!("Getting status for VM '{}'...", name);
            let output = Command::new("virsh")
                .args(&["dominfo", name])
                .output()?;
            
            if !output.status.success() {
                return Err(format!("virsh command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
            }
            
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            if format == "pretty" {
                println!("{}", stdout);
            } else {
                // Parse and format as JSON/YAML
                let info = parse_virsh_dominfo(&stdout)?;
                output_data(&info, format)?;
            }
        }
        
        "virtualbox" => {
            println!("Getting status for VM '{}'...", name);
            let output = Command::new("VBoxManage")
                .args(&["showvminfo", name])
                .output()?;
            
            if !output.status.success() {
                return Err(format!("VBoxManage command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
            }
            
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }
        
        _ => {
            return Err(format!("Unsupported hypervisor: {}", hypervisor).into());
        }
    }
    
    Ok(())
}

fn reboot_vm(name: &str, hypervisor: &str, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    match hypervisor {
        "kvm" | "qemu" => {
            let action = if force { "reset" } else { "reboot" };
            println!("{} VM '{}'...", if force { "Resetting" } else { "Rebooting" }, name);
            
            let output = Command::new("virsh")
                .args(&[action, name])
                .output()?;
            
            if output.status.success() {
                println!("✓ VM '{}' {} successfully", name, if force { "reset" } else { "reboot initiated" });
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to reboot VM: {}", error).into());
            }
        }
        
        "virtualbox" => {
            let action_type = if force { "reset" } else { "acpireboot" };
            println!("{} VM '{}'...", if force { "Resetting" } else { "Rebooting" }, name);
            
            let output = Command::new("VBoxManage")
                .args(&["controlvm", name, action_type])
                .output()?;
            
            if output.status.success() {
                println!("✓ VM '{}' {} successfully", name, if force { "reset" } else { "reboot initiated" });
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Failed to reboot VM: {}", error).into());
            }
        }
        
        _ => {
            return Err(format!("Unsupported hypervisor: {}", hypervisor).into());
        }
    }
    
    Ok(())
}

// Helper function to parse virsh list output
fn parse_virsh_list(output: &str) -> Result<Vec<VmInfo>, Box<dyn std::error::Error>> {
    let mut vms = Vec::new();
    
    for line in output.lines().skip(2) { // Skip header lines
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            vms.push(VmInfo {
                id: Some(parts[0].to_string()),
                name: parts[1].to_string(),
                state: parts[2..].join(" "),
                uuid: None,
            });
        }
    }
    
    Ok(vms)
}

// Helper function to parse virsh dominfo output
fn parse_virsh_dominfo(output: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut info = serde_json::Map::new();
    
    for line in output.lines() {
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim().to_lowercase().replace(' ', "_");
            let value = line[pos + 1..].trim();
            info.insert(key, serde_json::Value::String(value.to_string()));
        }
    }
    
    Ok(serde_json::Value::Object(info))
}
// Structures for VM inventory
#[derive(Debug, Serialize, Deserialize)]
struct VmInventory {
    host_mac_address: String,
    hypervisor_type: String,
    vms: Vec<VmDetail>,
}

#[derive(Debug, Serialize, Deserialize)]
struct VmDetail {
    vm_name: String,
    vm_uuid: Option<String>,
    vm_state: Option<String>,
    hypervisor_type: String,
    vcpu_count: Option<i32>,
    memory_mb: Option<i32>,
    guest_os_family: Option<String>,
    disks: Vec<VmDiskDetail>,
    network_interfaces: Vec<VmNetworkDetail>,
}

#[derive(Debug, Serialize, Deserialize)]
struct VmDiskDetail {
    disk_name: String,
    disk_type: Option<String>,
    disk_format: Option<String>,
    disk_size_gb: Option<i32>,
    disk_path: String,
    is_bootable: Option<bool>,
    storage_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct VmNetworkDetail {
    interface_name: String,
    mac_address: Option<String>,
    interface_type: Option<String>,
    network_bridge: Option<String>,
}

// Collect VM inventory from the system
/// Get the primary network interface MAC address of the host
fn get_host_primary_mac() -> Result<String, Box<dyn std::error::Error>> {
    let sys_class_net = Path::new("/sys/class/net");
    
    // Try to find the primary interface (non-virtual, has carrier)
    let entries = fs::read_dir(sys_class_net)?;
    
    for entry in entries.flatten() {
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };
        
        // Skip loopback and virtual interfaces
        if name.starts_with("lo") 
            || name.starts_with("veth") 
            || name.starts_with("docker") 
            || name.starts_with("br-") 
            || name.starts_with("virbr") {
            continue;
        }
        
        let iface_path = entry.path();
        
        // Check if interface has carrier (is connected)
        if let Ok(carrier) = fs::read_to_string(iface_path.join("carrier")) {
            if carrier.trim() == "1" {
                // This interface is connected, get its MAC address
                if let Ok(mac) = fs::read_to_string(iface_path.join("address")) {
                    let mac = mac.trim().to_string();
                    if !mac.is_empty() && mac != "00:00:00:00:00:00" {
                        return Ok(mac);
                    }
                }
            }
        }
    }
    
    Err("Could not find primary network interface MAC address".into())
}

fn collect_vm_inventory(hypervisor: &str) -> Result<VmInventory, Box<dyn std::error::Error>> {
    let host_mac = get_host_primary_mac()?;
    
    match hypervisor {
        "kvm" | "qemu" => collect_kvm_inventory(host_mac),
        "virtualbox" => collect_virtualbox_inventory(host_mac),
        _ => Err(format!("Unsupported hypervisor: {}", hypervisor).into()),
    }
}

// Collect KVM/QEMU VM inventory
fn collect_kvm_inventory(host_mac_address: String) -> Result<VmInventory, Box<dyn std::error::Error>> {
    let output = Command::new("virsh")
        .args(&["list", "--all", "--name"])
        .output()?;
    
    if !output.status.success() {
        return Err(format!("virsh list command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    let vm_names: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect();
    
    let mut vms = Vec::new();
    
    for vm_name in vm_names {
        if let Ok(vm_detail) = collect_kvm_vm_detail(&vm_name) {
            vms.push(vm_detail);
        }
    }
    
    Ok(VmInventory {
        host_mac_address,
        hypervisor_type: "KVM".to_string(),
        vms,
    })
}

// Collect detailed information for a single KVM VM
fn collect_kvm_vm_detail(vm_name: &str) -> Result<VmDetail, Box<dyn std::error::Error>> {
    // Get VM info
    let dominfo_output = Command::new("virsh")
        .args(&["dominfo", vm_name])
        .output()?;
    
    let dominfo = String::from_utf8_lossy(&dominfo_output.stdout);
    let mut vm_state = None;
    let mut vm_uuid = None;
    let mut vcpu_count = None;
    let mut memory_mb = None;
    
    for line in dominfo.lines() {
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim();
            let value = line[pos + 1..].trim();
            
            match key {
                "State" => vm_state = Some(normalize_vm_state(value)),
                "UUID" => vm_uuid = Some(value.to_string()),
                "CPU(s)" => vcpu_count = value.parse().ok(),
                "Max memory" => {
                    // Parse "1048576 KiB" format
                    if let Some(num_str) = value.split_whitespace().next() {
                        if let Ok(kb) = num_str.parse::<i32>() {
                            memory_mb = Some(kb / 1024);
                        }
                    }
                },
                _ => {}
            }
        }
    }
    
    // Get VM disks
    let disks = collect_kvm_vm_disks(vm_name)?;
    
    // Get VM network interfaces
    let network_interfaces = collect_kvm_vm_networks(vm_name)?;
    
    // Get guest OS info if possible
    let guest_os_family = detect_guest_os(vm_name);
    
    Ok(VmDetail {
        vm_name: vm_name.to_string(),
        vm_uuid,
        vm_state,
        hypervisor_type: "KVM".to_string(),
        vcpu_count,
        memory_mb,
        guest_os_family,
        disks,
        network_interfaces,
    })
}

// Collect disk information for a KVM VM
fn collect_kvm_vm_disks(vm_name: &str) -> Result<Vec<VmDiskDetail>, Box<dyn std::error::Error>> {
    let output = Command::new("virsh")
        .args(&["domblklist", vm_name, "--details"])
        .output()?;
    
    if !output.status.success() {
        return Ok(Vec::new());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut disks = Vec::new();
    
    for line in stdout.lines().skip(2) { // Skip header lines
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let disk_type = parts[0].to_string(); // e.g., "file"
            let device = parts[1].to_string(); // e.g., "disk"
            let target = parts[2].to_string(); // e.g., "vda"
            let source = parts[3].to_string(); // disk path
            
            // Try to get disk size
            let disk_size_gb = get_disk_size(&source).unwrap_or(0); // Default to 0 if size cannot be determined
            
            // Determine disk format from file extension or qemu-img
            let disk_format = detect_disk_format(&source);
            
            disks.push(VmDiskDetail {
                disk_name: target.clone(),
                disk_type: Some(detect_disk_type(&target)),
                disk_format: Some(disk_format),
                disk_size_gb: Some(disk_size_gb),
                disk_path: source,
                is_bootable: Some(disks.is_empty()), // First disk is usually bootable
                storage_type: Some(if disk_type == "file" { "file".to_string() } else { "block".to_string() }),
            });
        }
    }
    
    Ok(disks)
}

// Collect network interface information for a KVM VM
fn collect_kvm_vm_networks(vm_name: &str) -> Result<Vec<VmNetworkDetail>, Box<dyn std::error::Error>> {
    let output = Command::new("virsh")
        .args(&["domiflist", vm_name])
        .output()?;
    
    if !output.status.success() {
        return Ok(Vec::new());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut interfaces = Vec::new();
    
    for line in stdout.lines().skip(2) { // Skip header lines
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let interface_name = parts[0].to_string();
            let interface_type = parts[1].to_string();
            let source = parts[2].to_string();
            let mac_address = parts[3].to_string();
            
            interfaces.push(VmNetworkDetail {
                interface_name: interface_name.clone(),
                mac_address: Some(mac_address),
                interface_type: Some(normalize_interface_type(&interface_type)),
                network_bridge: Some(source),
            });
        }
    }
    
    Ok(interfaces)
}

// Helper functions
fn normalize_interface_type(interface_type: &str) -> String {
    // Map virsh interface types to database ENUM values
    // Database ENUM: 'bridge', 'nat', 'host-only', 'internal', 'external'
    let type_lower = interface_type.to_lowercase();
    
    match type_lower.as_str() {
        "bridge" => "bridge".to_string(),
        "network" => "nat".to_string(), // virsh 'network' usually means NAT
        "nat" => "nat".to_string(),
        "direct" | "macvtap" => "external".to_string(),
        "hostdev" | "ethernet" => "external".to_string(),
        "internal" => "internal".to_string(),
        "isolated" | "private" => "host-only".to_string(),
        _ => "bridge".to_string(), // Default to bridge
    }
}

fn detect_disk_type(target: &str) -> String {
    if target.starts_with("vd") {
        "virtio".to_string()
    } else if target.starts_with("sd") {
        "scsi".to_string()
    } else if target.starts_with("hd") {
        "ide".to_string()
    } else if target.starts_with("nvme") {
        "nvme".to_string()
    } else {
        "virtio".to_string()
    }
}

fn detect_disk_format(path: &str) -> String {
    if path.ends_with(".qcow2") {
        "qcow2".to_string()
    } else if path.ends_with(".raw") {
        "raw".to_string()
    } else if path.ends_with(".vmdk") {
        "vmdk".to_string()
    } else if path.ends_with(".vdi") {
        "vdi".to_string()
    } else {
        // Try to detect using qemu-img
        let output = Command::new("qemu-img")
            .args(&["info", path])
            .output();
        
        if let Ok(output) = output {
            let info = String::from_utf8_lossy(&output.stdout);
            for line in info.lines() {
                if line.starts_with("file format:") {
                    if let Some(format) = line.split(':').nth(1) {
                        return format.trim().to_string();
                    }
                }
            }
        }
        
        "qcow2".to_string() // Default
    }
}

fn get_disk_size(path: &str) -> Option<i32> {
    let output = Command::new("qemu-img")
        .args(&["info", "--output=json", path])
        .output()
        .ok()?;
    
    if let Ok(json_str) = String::from_utf8(output.stdout) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(size) = json["virtual-size"].as_i64() {
                return Some((size / 1024 / 1024 / 1024) as i32);
            }
        }
    }
    
    None
}

fn normalize_vm_state(state: &str) -> String {
    // Map virsh states to database ENUM values
    // Database ENUM: 'running', 'stopped', 'paused', 'suspended', 'crashed', 'unknown'
    let state_lower = state.to_lowercase();
    
    match state_lower.as_str() {
        "running" => "running".to_string(),
        "shut off" | "shutoff" | "stopped" => "stopped".to_string(),
        "paused" => "paused".to_string(),
        "suspended" | "pmsuspended" | "saved" => "suspended".to_string(),
        "crashed" | "dying" => "crashed".to_string(),
        "idle" | "blocked" | "in shutdown" => "running".to_string(), // Treat these as running
        _ => "unknown".to_string(),
    }
}

fn detect_guest_os(vm_name: &str) -> Option<String> {
    // Try to detect from VM name patterns
    let name_lower = vm_name.to_lowercase();
    
    if name_lower.contains("ubuntu") || name_lower.contains("debian") {
        Some("Linux".to_string())
    } else if name_lower.contains("centos") || name_lower.contains("rhel") || name_lower.contains("fedora") {
        Some("Linux".to_string())
    } else if name_lower.contains("windows") || name_lower.contains("win") {
        Some("Windows".to_string())
    } else if name_lower.contains("freebsd") {
        Some("FreeBSD".to_string())
    } else {
        None
    }
}

// Collect VirtualBox VM inventory (basic implementation)
fn collect_virtualbox_inventory(host_mac_address: String) -> Result<VmInventory, Box<dyn std::error::Error>> {
    let output = Command::new("VBoxManage")
        .args(&["list", "vms"])
        .output()?;
    
    if !output.status.success() {
        return Err(format!("VBoxManage command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    // Parse VirtualBox VM list (simplified)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut vms = Vec::new();
    
    for line in stdout.lines() {
        // VirtualBox format: "name" {uuid}
        if let Some(start) = line.find('"') {
            if let Some(end) = line[start + 1..].find('"') {
                let vm_name = &line[start + 1..start + 1 + end];
                
                // For VirtualBox, we'd need more detailed parsing
                // This is a simplified version
                vms.push(VmDetail {
                    vm_name: vm_name.to_string(),
                    vm_uuid: None,
                    vm_state: None,
                    hypervisor_type: "VirtualBox".to_string(),
                    vcpu_count: None,
                    memory_mb: None,
                    guest_os_family: None,
                    disks: Vec::new(),
                    network_interfaces: Vec::new(),
                });
            }
        }
    }
    
    Ok(VmInventory {
        host_mac_address,
        hypervisor_type: "VirtualBox".to_string(),
        vms,
    })
}
