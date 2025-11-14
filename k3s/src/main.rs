
// main.rs
use anyhow::Result;
use clap::{Parser, Subcommand};

mod prod_cluster;
mod utils;

#[derive(Parser)]
#[command(name = "k3d-manager")]
#[command(about = "Manage multiple k3d clusters", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a simple development cluster
    Dev {
        /// Cluster name
        #[arg(short, long, default_value = "dev-cluster")]
        name: String,
        
        /// Number of worker nodes
        #[arg(short, long, default_value = "2")]
        workers: u8,
    },
    
    /// Create a production-like cluster with full stack
    Prod {
        /// Cluster name
        #[arg(short, long, default_value = "prod-cluster")]
        name: String,
        
        /// Number of control plane nodes
        #[arg(short, long, default_value = "3")]
        servers: u8,
        
        /// Number of worker nodes
        #[arg(short = 'w', long, default_value = "3")]
        agents: u8,
        
        /// Skip monitoring stack installation
        #[arg(long)]
        skip_monitoring: bool,
        
        /// Skip logging stack installation
        #[arg(long)]
        skip_logging: bool,
        
        /// Skip ArgoCD installation
        #[arg(long)]
        skip_argocd: bool,
    },
    
    /// List all k3d clusters
    List,
    
    /// Delete a cluster
    Delete {
        /// Cluster name
        name: String,
    },
    
    /// Get cluster info
    Info {
        /// Cluster name
        name: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Dev { name, workers } => {
            create_dev_cluster(&name, workers).await?;
        }
        Commands::Prod { 
            name, 
            servers, 
            agents,
            skip_monitoring,
            skip_logging,
            skip_argocd,
        } => {
            let config = prod_cluster::ProdClusterConfig {
                name,
                servers,
                agents,
                install_monitoring: !skip_monitoring,
                install_logging: !skip_logging,
                install_argocd: !skip_argocd,
            };
            prod_cluster::create_prod_cluster(config).await?;
        }
        Commands::List => {
            list_clusters()?;
        }
        Commands::Delete { name } => {
            delete_cluster(&name)?;
        }
        Commands::Info { name } => {
            cluster_info(&name)?;
        }
    }
    
    Ok(())
}

async fn create_dev_cluster(name: &str, workers: u8) -> Result<()> {
    println!("🚀 Creating dev cluster: {}", name);
    println!("   Workers: {}", workers);
    
    utils::run("k3d", &[
        "cluster", "create", name,
        "--servers", "1",
        "--agents", &workers.to_string(),
        "--port", "8080:80@loadbalancer",
        "--port", "8443:443@loadbalancer",
        "--wait",
    ])?;
    
    println!("✅ Dev cluster '{}' created successfully!", name);
    println!("\n📋 Quick commands:");
    println!("   kubectl get nodes");
    println!("   kubectl config use-context k3d-{}", name);
    println!("   k3d cluster delete {}", name);
    
    Ok(())
}

fn list_clusters() -> Result<()> {
    println!("📋 K3D Clusters:\n");
    utils::run("k3d", &["cluster", "list"])?;
    Ok(())
}

fn delete_cluster(name: &str) -> Result<()> {
    println!("🗑️  Deleting cluster: {}", name);
    utils::run("k3d", &["cluster", "delete", name])?;
    println!("✅ Cluster '{}' deleted", name);
    Ok(())
}

fn cluster_info(name: &str) -> Result<()> {
    println!("ℹ️  Cluster Info: {}\n", name);
    
    // Set context
    utils::run("kubectl", &[
        "config", "use-context", &format!("k3d-{}", name)
    ])?;
    
    println!("\n📦 Nodes:");
    utils::run("kubectl", &["get", "nodes", "-o", "wide"])?;
    
    println!("\n📊 All Pods:");
    utils::run("kubectl", &["get", "pods", "-A"])?;
    
    println!("\n🌐 Services:");
    utils::run("kubectl", &["get", "svc", "-A"])?;
    
    Ok(())
}