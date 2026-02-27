use crate::hardware::types::Inventory;
use crate::hardware;

const AGENT_VERSION: &str = "1.0.0";

pub fn collect_full_inventory() -> Inventory {
    let node = hardware::collect_node_info();
    let cpu = hardware::collect_cpu_info();
    let memory = hardware::collect_memory_info();
    let disks = hardware::collect_disks();
    let network = hardware::collect_network_info();
    let gpus = hardware::collect_gpus();
    let power_supplies = hardware::collect_power_supplies();

    Inventory {
        agent_version: AGENT_VERSION.to_string(),
        node,
        cpu,
        memory,
        disks,
        network,
        gpus,
        power_supplies,
    }
}