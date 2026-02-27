use crate::cli::K8sCommands;
use crate::output::output_data;
use std::io::{self, Write};
use std::process::Command;

pub fn handle_k8s_command(cmd: &K8sCommands) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        K8sCommands::Pods { namespace, all_namespaces, format } => {
            list_pods(namespace.as_deref(), *all_namespaces, format)?;
        }
        
        K8sCommands::Deployments { namespace, all_namespaces, format } => {
            list_deployments(namespace.as_deref(), *all_namespaces, format)?;
        }
        
        K8sCommands::Services { namespace, all_namespaces, format } => {
            list_services(namespace.as_deref(), *all_namespaces, format)?;
        }
        
        K8sCommands::Nodes { format } => {
            list_nodes(format)?;
        }
        
        K8sCommands::Namespaces { format } => {
            list_namespaces(format)?;
        }
        
        K8sCommands::Apply { file, namespace } => {
            apply_manifest(file, namespace.as_deref())?;
        }
        
        K8sCommands::Delete { resource_type, name, namespace, yes } => {
            delete_resource(resource_type, name, namespace.as_deref(), *yes)?;
        }
        
        K8sCommands::Scale { name, replicas, namespace } => {
            scale_deployment(name, *replicas, namespace.as_deref())?;
        }
        
        K8sCommands::Logs { name, namespace, container, follow, tail } => {
            get_logs(name, namespace.as_deref(), container.as_deref(), *follow, *tail)?;
        }
        
        K8sCommands::Exec { name, namespace, container, command } => {
            exec_in_pod(name, namespace.as_deref(), container.as_deref(), command)?;
        }
        
        K8sCommands::ClusterInfo { format } => {
            cluster_info(format)?;
        }
        
        K8sCommands::Describe { resource_type, name, namespace } => {
            describe_resource(resource_type, name, namespace.as_deref())?;
        }
    }
    Ok(())
}

fn list_pods(namespace: Option<&str>, all_namespaces: bool, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["get", "pods"];
    
    if all_namespaces {
        args.push("--all-namespaces");
    } else if let Some(ns) = namespace {
        args.push("-n");
        args.push(ns);
    } else {
        args.push("--all-namespaces");
    }
    
    match format {
        "json" => args.push("-o=json"),
        "yaml" => args.push("-o=yaml"),
        "wide" => args.push("-o=wide"),
        _ => {} // default table format
    }
    
    execute_kubectl(&args, format)
}

fn list_deployments(namespace: Option<&str>, all_namespaces: bool, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["get", "deployments"];
    
    if all_namespaces {
        args.push("--all-namespaces");
    } else if let Some(ns) = namespace {
        args.push("-n");
        args.push(ns);
    } else {
        args.push("--all-namespaces");
    }
    
    match format {
        "json" => args.push("-o=json"),
        "yaml" => args.push("-o=yaml"),
        "wide" => args.push("-o=wide"),
        _ => {}
    }
    
    execute_kubectl(&args, format)
}

fn list_services(namespace: Option<&str>, all_namespaces: bool, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["get", "services"];
    
    if all_namespaces {
        args.push("--all-namespaces");
    } else if let Some(ns) = namespace {
        args.push("-n");
        args.push(ns);
    } else {
        args.push("--all-namespaces");
    }
    
    match format {
        "json" => args.push("-o=json"),
        "yaml" => args.push("-o=yaml"),
        "wide" => args.push("-o=wide"),
        _ => {}
    }
    
    execute_kubectl(&args, format)
}

fn list_nodes(format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["get", "nodes"];
    
    match format {
        "json" => args.push("-o=json"),
        "yaml" => args.push("-o=yaml"),
        "wide" => args.push("-o=wide"),
        _ => {}
    }
    
    execute_kubectl(&args, format)
}

fn list_namespaces(format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["get", "namespaces"];
    
    match format {
        "json" => args.push("-o=json"),
        "yaml" => args.push("-o=yaml"),
        _ => {}
    }
    
    execute_kubectl(&args, format)
}

fn apply_manifest(file: &str, namespace: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["apply", "-f", file];
    
    if let Some(ns) = namespace {
        args.push("-n");
        args.push(ns);
    }
    
    println!("Applying manifest from: {}", file);
    
    let output = Command::new("kubectl")
        .args(&args)
        .output()?;
    
    if output.status.success() {
        println!("✓ Manifest applied successfully");
        println!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to apply manifest: {}", error).into());
    }
    
    Ok(())
}

fn delete_resource(resource_type: &str, name: &str, namespace: Option<&str>, yes: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !yes {
        print!("Are you sure you want to delete {} '{}'? [y/N]: ", resource_type, name);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }
    
    let mut args = vec!["delete", resource_type, name];
    
    if let Some(ns) = namespace {
        args.push("-n");
        args.push(ns);
    }
    
    println!("Deleting {} '{}'...", resource_type, name);
    
    let output = Command::new("kubectl")
        .args(&args)
        .output()?;
    
    if output.status.success() {
        println!("✓ {} '{}' deleted successfully", resource_type, name);
        println!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to delete resource: {}", error).into());
    }
    
    Ok(())
}

fn scale_deployment(name: &str, replicas: u32, namespace: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let replicas_str = replicas.to_string();
    let mut args = vec!["scale", "deployment", name, "--replicas", &replicas_str];
    
    if let Some(ns) = namespace {
        args.push("-n");
        args.push(ns);
    }
    
    println!("Scaling deployment '{}' to {} replicas...", name, replicas);
    
    let output = Command::new("kubectl")
        .args(&args)
        .output()?;
    
    if output.status.success() {
        println!("✓ Deployment '{}' scaled successfully", name);
        println!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to scale deployment: {}", error).into());
    }
    
    Ok(())
}

fn get_logs(name: &str, namespace: Option<&str>, container: Option<&str>, follow: bool, tail: Option<u32>) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["logs", name];
    
    if let Some(ns) = namespace {
        args.push("-n");
        args.push(ns);
    }
    
    if let Some(c) = container {
        args.push("-c");
        args.push(c);
    }
    
    if follow {
        args.push("-f");
    }
    
    let tail_str;
    if let Some(t) = tail {
        args.push("--tail");
        tail_str = t.to_string();
        args.push(&tail_str);
    }
    
    println!("Getting logs for pod '{}'...", name);
    
    let output = Command::new("kubectl")
        .args(&args)
        .output()?;
    
    if output.status.success() {
        println!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to get logs: {}", error).into());
    }
    
    Ok(())
}

fn exec_in_pod(name: &str, namespace: Option<&str>, container: Option<&str>, command: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["exec", "-it", name];
    
    if let Some(ns) = namespace {
        args.push("-n");
        args.push(ns);
    }
    
    if let Some(c) = container {
        args.push("-c");
        args.push(c);
    }
    
    args.push("--");
    
    // Convert Vec<String> to Vec<&str>
    let cmd_refs: Vec<&str> = command.iter().map(|s| s.as_str()).collect();
    args.extend(cmd_refs);
    
    println!("Executing command in pod '{}'...", name);
    
    let status = Command::new("kubectl")
        .args(&args)
        .status()?;
    
    if !status.success() {
        return Err("Command execution failed".into());
    }
    
    Ok(())
}

fn cluster_info(format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let args = if format == "json" || format == "yaml" {
        vec!["cluster-info", "dump"]
    } else {
        vec!["cluster-info"]
    };
    
    execute_kubectl(&args, format)
}

fn describe_resource(resource_type: &str, name: &str, namespace: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = vec!["describe", resource_type, name];
    
    if let Some(ns) = namespace {
        args.push("-n");
        args.push(ns);
    }
    
    println!("Describing {} '{}'...", resource_type, name);
    
    let output = Command::new("kubectl")
        .args(&args)
        .output()?;
    
    if output.status.success() {
        println!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to describe resource: {}", error).into());
    }
    
    Ok(())
}

fn execute_kubectl(args: &[&str], format: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("kubectl")
        .args(args)
        .output()?;
    
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("kubectl command failed: {}", error).into());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // For JSON/YAML, parse and use output_data
    if format == "json" {
        let json_value: serde_json::Value = serde_json::from_str(&stdout)?;
        output_data(&json_value, format)?;
    } else if format == "yaml" {
        // Just print YAML as-is
        println!("{}", stdout);
    } else {
        // Pretty/table format
        println!("{}", stdout);
    }
    
    Ok(())
}
