use anyhow::{Context, Result};
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    let cluster_name = "rusty-cluster";
    println!("🚀 Creating k3d cluster: {}", cluster_name);

    // Create the 3-node cluster: 1 control plane + 2 workers
    run("k3d", &[
        "cluster", "create", cluster_name,
        "--servers", "3",
        "--agents", "3",
        "--wait",
    ])?;

    println!("✅ Cluster created successfully! Waiting for nodes to be ready...");

    // Give the cluster a few seconds to settle before querying
    sleep(Duration::from_secs(10)).await;

    // Show nodes
    run("kubectl", &["get", "nodes", "-o", "wide"])?;

    // Example: apply a simple workload or test manifest (optional)
    // run("kubectl", &["apply", "-f", "your-agent.yaml"])?;

    println!("🎉 Cluster is healthy and ready to test your SRE agent!");

    // Uncomment if you want to automatically delete the cluster when done:
    // println!("🧹 Cleaning up...");
    // run("k3d", &["cluster", "delete", cluster_name])?;

    Ok(())
}

fn run(cmd: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("failed to run command: {} {:?}", cmd, args))?;

    if !output.status.success() {
        anyhow::bail!(
            "command failed: {} {:?}\nstdout: {}\nstderr: {}",
            cmd,
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    println!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}
