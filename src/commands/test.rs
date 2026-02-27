use crate::cli::TestCommands;
use crate::testing::{
    collect_gpu_errors,
    collect_gpu_health,
    collect_nccl_info,
    run_nccl_test,
    collect_mpi_info,
    run_mpi_test,
    collect_hashcat_info,
    run_hashcat_benchmark,
    run_hashcat_test,
    collect_dcgm_info,
    run_dcgm_diag,
    run_dcgm_health_check,
};
use crate::output::output_data;

pub fn handle_test_command(cmd: &TestCommands) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        TestCommands::GpuErrors { format } => {
            match collect_gpu_errors() {
                Ok(gpu_errors) => {
                    output_data(&gpu_errors, format)?;
                }
                Err(e) => {
                    eprintln!("✗ Error collecting GPU errors: {}", e);
                    eprintln!("Note: This command requires NVIDIA GPUs with NVML support.");
                    return Err(e);
                }
            }
        }
        TestCommands::GpuHealth { format } => {
            match collect_gpu_health() {
                Ok(gpu_health) => {
                    output_data(&gpu_health, format)?;
                }
                Err(e) => {
                    eprintln!("✗ Error collecting GPU health: {}", e);
                    eprintln!("Note: This command requires NVIDIA GPUs with NVML support.");
                    return Err(e);
                }
            }
        }
        TestCommands::NcclInfo { format } => {
            let nccl_info = collect_nccl_info();
            output_data(&nccl_info, format)?;
        }
        TestCommands::NcclTest { test_type, size, iterations, format } => {
            match run_nccl_test(test_type, size, *iterations) {
                Ok(test_result) => {
                    output_data(&test_result, format)?;
                }
                Err(e) => {
                    eprintln!("✗ Error running NCCL test: {}", e);
                    eprintln!("Note: This command requires NVIDIA GPUs and NCCL installation.");
                    return Err(e);
                }
            }
        }
        TestCommands::MpiInfo { format } => {
            let mpi_info = collect_mpi_info();
            output_data(&mpi_info, format)?;
        }
        TestCommands::MpiTest { test_type, processes, size, iterations, format } => {
            match run_mpi_test(test_type, *processes, size, *iterations) {
                Ok(test_result) => {
                    output_data(&test_result, format)?;
                }
                Err(e) => {
                    eprintln!("✗ Error running MPI test: {}", e);
                    eprintln!("Note: This command requires MPI installation (OpenMPI, MPICH, etc.).");
                    return Err(e);
                }
            }
        }
        TestCommands::HashcatInfo { format } => {
            let hashcat_info = collect_hashcat_info();
            output_data(&hashcat_info, format)?;
        }
        TestCommands::HashcatBenchmark { hash_types, devices, format } => {
            match run_hashcat_benchmark(hash_types.clone(), devices.clone()) {
                Ok(results) => {
                    output_data(&results, format)?;
                }
                Err(e) => {
                    eprintln!("✗ Error running Hashcat benchmark: {}", e);
                    eprintln!("Note: This command requires Hashcat installation.");
                    return Err(e);
                }
            }
        }
        TestCommands::HashcatTest { hash_type, hash_file, wordlist, devices, format } => {
            match run_hashcat_test(hash_type, hash_file, wordlist, devices.clone()) {
                Ok(test_result) => {
                    output_data(&test_result, format)?;
                }
                Err(e) => {
                    eprintln!("✗ Error running Hashcat test: {}", e);
                    eprintln!("Note: This command requires Hashcat installation.");
                    return Err(e);
                }
            }
        }
        TestCommands::DcgmInfo { format } => {
            let dcgm_info = collect_dcgm_info();
            output_data(&dcgm_info, format)?;
        }
        TestCommands::DcgmDiag { level, gpus, format } => {
            match run_dcgm_diag(*level, gpus.clone()) {
                Ok(diag_result) => {
                    output_data(&diag_result, format)?;
                }
                Err(e) => {
                    eprintln!("✗ Error running DCGM diagnostics: {}", e);
                    eprintln!("Note: This command requires DCGM installation and NVIDIA GPUs.");
                    return Err(e);
                }
            }
        }
        TestCommands::DcgmHealth { format } => {
            match run_dcgm_health_check() {
                Ok(health_results) => {
                    output_data(&health_results, format)?;
                }
                Err(e) => {
                    eprintln!("✗ Error running DCGM health check: {}", e);
                    eprintln!("Note: This command requires DCGM installation and NVIDIA GPUs.");
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}
