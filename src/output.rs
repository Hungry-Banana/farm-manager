use serde::Serialize;

pub fn output_data<T: Serialize>(data: &T, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(data)?);
        }
        "yaml" => {
            println!("{}", serde_yaml::to_string(data)?);
        }
        "pretty" | _ => {
            println!("{}", serde_json::to_string_pretty(data)?);
        }
    }
    Ok(())
}

pub fn print_success(message: &str) {
    println!("✅ {}", message);
}

pub fn print_error(message: &str) {
    eprintln!("\x1b[31m❌ Error: {}\x1b[0m", message);
}

pub fn print_warning(message: &str) {
    println!("\x1b[33m⚠️  Warning: {}\x1b[0m", message);
}

pub fn print_info(message: &str) {
    println!("ℹ️  {}", message);
}

pub fn confirm_action(message: &str) -> bool {
    println!("⚠️  {}", message);
    print!("Continue? [y/N]: ");
    use std::io::{self, Write};
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}