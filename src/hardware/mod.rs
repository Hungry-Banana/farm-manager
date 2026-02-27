// Hardware inventory collection modules
pub mod types;
pub mod collect_memory;
pub mod collect_cpu;
pub mod collect_network;
pub mod collect_storage;
pub mod collect_gpus;
pub mod collect_node;
pub mod collect_power;
pub mod collector;

// Re-export main collection functions
pub use collect_memory::collect_memory_info;
pub use collect_cpu::collect_cpu_info;
pub use collect_network::collect_network_info;
pub use collect_storage::collect_disks;
pub use collect_gpus::collect_gpus;
pub use collect_node::collect_node_info;
pub use collect_power::collect_power_supplies;
pub use collector::collect_full_inventory;