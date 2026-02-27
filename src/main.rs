mod hardware;
mod testing;
mod cli;
mod commands;
mod output;

use clap::Parser;
use cli::{Cli, Commands};
use commands::{
    handle_hardware_command,
    handle_test_command,
    handle_vm_command,
    handle_k8s_command,
};
use output::print_error;

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Hardware(cmd) => handle_hardware_command(cmd),
        Commands::Test(cmd) => handle_test_command(cmd),
        Commands::Vm(cmd) => handle_vm_command(cmd),
        Commands::K8s(cmd) => handle_k8s_command(cmd),
    };

    if let Err(e) = result {
        print_error(&e.to_string());
        std::process::exit(1);
    }
}