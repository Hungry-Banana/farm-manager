use crate::cli::HardwareCommands;
use crate::hardware::{
    collect_full_inventory,
    collect_memory_info,
    collect_cpu_info,
    collect_network_info,
    collect_disks,
    collect_node_info,
    collect_power_supplies,
};
use crate::output::output_data;

pub fn handle_hardware_command(cmd: &HardwareCommands) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        HardwareCommands::Inventory { format } => {
            let inventory = collect_full_inventory();
            output_data(&inventory, format)?;
        }
        HardwareCommands::Cpu { format } => {
            let cpu_info = collect_cpu_info();
            output_data(&cpu_info, format)?;
        }
        HardwareCommands::Memory { format } => {
            let memory_info = collect_memory_info();
            output_data(&memory_info, format)?;
        }
        HardwareCommands::Storage { format } => {
            let storage_info = collect_disks();
            output_data(&storage_info, format)?;
        }
        HardwareCommands::Network { format } => {
            let network_info = collect_network_info();
            output_data(&network_info, format)?;
        }
        HardwareCommands::Node { format } => {
            let node_info = collect_node_info();
            output_data(&node_info, format)?;
        }
        HardwareCommands::Power { format } => {
            let power_info = collect_power_supplies();
            output_data(&power_info, format)?;
        }
        HardwareCommands::PostInventory { url } => {
            println!("Collecting hardware inventory...");
            let inventory = collect_full_inventory();
            
            let api_url = format!("{}/api/v1/servers/inventory", url.trim_end_matches('/'));
            println!("Posting inventory to: {}", api_url);
            
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
                return Err(format!("Failed to post inventory: HTTP {}", status).into());
            }
        }
    }
    Ok(())
}