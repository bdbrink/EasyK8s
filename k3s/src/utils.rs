use anyhow::{Context, Result};
use std::process::Command;
use std::io::Write;

pub fn run(cmd: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("failed to run: {} {:?}", cmd, args))?;

    if !output.status.success() {
        anyhow::bail!(
            "Command failed: {} {:?}\nstderr: {}",
            cmd,
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        println!("{}", stdout);
    }
    
    Ok(())
}

pub fn apply_manifest(manifest: &str) -> Result<()> {
    let mut child = Command::new("kubectl")
        .args(&["apply", "-f", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn kubectl")?;
    
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(manifest.as_bytes())
            .context("Failed to write manifest to kubectl stdin")?;
    }
    
    let output = child.wait_with_output()
        .context("Failed to wait for kubectl")?;
    
    if !output.status.success() {
        anyhow::bail!(
            "kubectl apply failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    
    Ok(())
}