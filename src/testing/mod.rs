// Testing and diagnostics modules
pub mod gpu_errors;
pub mod nccl;
pub mod mpi;
pub mod hashcat;
pub mod dcgm;

// Re-export main collection functions
pub use gpu_errors::{collect_gpu_errors, collect_gpu_health};
pub use nccl::{collect_nccl_info, run_nccl_test};
pub use mpi::{collect_mpi_info, run_mpi_test};
pub use hashcat::{collect_hashcat_info, run_hashcat_benchmark, run_hashcat_test};
pub use dcgm::{collect_dcgm_info, run_dcgm_diag, run_dcgm_health_check};
