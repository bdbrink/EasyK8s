// prod_cluster.rs
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::sleep;
use std::fs;
use std::path::Path;
use crate::utils;

pub struct ProdClusterConfig {
    pub name: String,
    pub servers: u8,
    pub agents: u8,
    pub install_monitoring: bool,
    pub install_logging: bool,
    pub install_argocd: bool,
}

// Path to helm values directory
const HELM_VALUES_DIR: &str = "./helm-values";

pub async fn create_prod_cluster(config: ProdClusterConfig) -> Result<()> {
    println!("🚀 Building production-like k3d cluster: {}", config.name);
    println!("📋 Configuration:");
    println!("   Control Plane Nodes: {}", config.servers);
    println!("   Worker Nodes: {}", config.agents);
    println!("   Monitoring: {}", if config.install_monitoring { "✓" } else { "✗" });
    println!("   Logging: {}", if config.install_logging { "✓" } else { "✗" });
    println!("   ArgoCD: {}", if config.install_argocd { "✓" } else { "✗" });

    // Verify helm values directory exists
    ensure_helm_values_dir()?;

    // Create k3d config file
    create_k3d_config(&config)?;
    
    // Create the cluster with custom config
    println!("\n🏗️  Creating HA cluster...");
    utils::run("k3d", &[
        "cluster", "create", &config.name,
        "--config", "/tmp/k3d-prod-config.yaml",
    ])?;

    println!("✅ Cluster created! Waiting for nodes...");
    sleep(Duration::from_secs(10)).await;

    // Verify cluster health
    utils::run("kubectl", &["get", "nodes", "-o", "wide"])?;
    
    println!("\n📦 Installing core components via Helm...");
    
    // Add Helm repositories
    setup_helm_repos().await?;
    
    // Install core infrastructure
    install_cert_manager_helm().await?;
    install_ingress_controller_helm().await?;
    
    if config.install_monitoring {
        install_monitoring_stack_helm().await?;
    }
    
    if config.install_logging {
        install_logging_stack_helm().await?;
    }
    
    if config.install_argocd {
        install_argocd_helm().await?;
    }
    
    // Setup namespaces and policies
    setup_namespaces()?;
    setup_network_policies()?;
    setup_resource_quotas()?;
    
    // Deploy sample application
    deploy_sample_app_helm().await?;

    println!("\n🎉 Production cluster '{}' is ready!", config.name);
    print_access_info(&config.name);
    
    Ok(())
}

fn ensure_helm_values_dir() -> Result<()> {
    let helm_dir = Path::new(HELM_VALUES_DIR);
    if !helm_dir.exists() {
        anyhow::bail!(
            "Helm values directory not found at '{}'\n\
            Please create it with: mkdir -p {}\n\
            Or run: cargo run --bin setup-helm-values",
            HELM_VALUES_DIR, HELM_VALUES_DIR
        );
    }
    Ok(())
}

fn get_values_file(component: &str) -> String {
    format!("{}/{}.yaml", HELM_VALUES_DIR, component)
}

fn create_k3d_config(config: &ProdClusterConfig) -> Result<()> {
    let yaml_config = format!(r#"
apiVersion: k3d.io/v1alpha5
kind: Simple
metadata:
  name: {}
servers: {}
agents: {}
image: rancher/k3s:v1.28.5-k3s1

ports:
  - port: 80:80
    nodeFilters:
      - loadbalancer
  - port: 443:443
    nodeFilters:
      - loadbalancer
  - port: 9090:9090
    nodeFilters:
      - loadbalancer
  - port: 3000:3000
    nodeFilters:
      - loadbalancer
  - port: 8080:8080
    nodeFilters:
      - loadbalancer
  - port: 5601:5601
    nodeFilters:
      - loadbalancer

volumes:
  - volume: /tmp/k3d-storage:/var/lib/rancher/k3s/storage
    nodeFilters:
      - all

registries:
  create:
    name: registry.localhost
    host: "0.0.0.0"
    hostPort: "5000"

options:
  k3s:
    extraArgs:
      - arg: --disable=traefik
        nodeFilters:
          - server:*
      - arg: --disable=servicelb
        nodeFilters:
          - server:*
  kubeconfig:
    updateDefaultKubeconfig: true
    switchCurrentContext: true
"#, config.name, config.servers, config.agents);

    fs::write("/tmp/k3d-prod-config.yaml", yaml_config)
        .context("Failed to write k3d config")?;
    
    println!("✅ Created k3d configuration");
    Ok(())
}

async fn setup_helm_repos() -> Result<()> {
    println!("\n📚 Adding Helm repositories...");
    
    let repos = vec![
        ("jetstack", "https://charts.jetstack.io"),
        ("ingress-nginx", "https://kubernetes.github.io/ingress-nginx"),
        ("prometheus-community", "https://prometheus-community.github.io/helm-charts"),
        ("elastic", "https://helm.elastic.co"),
        ("fluent", "https://fluent.github.io/helm-charts"),
        ("argo", "https://argoproj.github.io/argo-helm"),
    ];
    
    for (name, url) in repos {
        utils::run("helm", &["repo", "add", name, url])?;
    }
    
    utils::run("helm", &["repo", "update"])?;
    
    println!("✅ Helm repositories configured");
    Ok(())
}

async fn install_cert_manager_helm() -> Result<()> {
    println!("\n🔐 Installing cert-manager via Helm...");
    
    utils::run("kubectl", &["create", "namespace", "cert-manager"])?;
    
    let values_file = get_values_file("cert-manager");
    
    utils::run("helm", &[
        "install", "cert-manager", "jetstack/cert-manager",
        "--namespace", "cert-manager",
        "--version", "v1.13.2",
        "--values", &values_file,
    ])?;
    
    sleep(Duration::from_secs(30)).await;
    
    utils::run("kubectl", &[
        "wait", "--for=condition=ready", "pod",
        "-l", "app.kubernetes.io/instance=cert-manager",
        "-n", "cert-manager",
        "--timeout=300s"
    ])?;
    
    // Create self-signed issuer
    let issuer = r#"
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: selfsigned-issuer
spec:
  selfSigned: {}
---
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: local-ca
  namespace: cert-manager
spec:
  isCA: true
  commonName: local-ca
  secretName: local-ca-secret
  privateKey:
    algorithm: ECDSA
    size: 256
  issuerRef:
    name: selfsigned-issuer
    kind: ClusterIssuer
---
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: local-ca-issuer
spec:
  ca:
    secretName: local-ca-secret
"#;
    
    utils::apply_manifest(issuer)?;
    println!("✅ Cert-manager installed via Helm");
    Ok(())
}

async fn install_ingress_controller_helm() -> Result<()> {
    println!("\n🌐 Installing NGINX Ingress via Helm...");
    
    utils::run("kubectl", &["create", "namespace", "ingress-nginx"])?;
    
    let values_file = get_values_file("ingress-nginx");
    
    utils::run("helm", &[
        "install", "ingress-nginx", "ingress-nginx/ingress-nginx",
        "--namespace", "ingress-nginx",
        "--values", &values_file,
    ])?;
    
    sleep(Duration::from_secs(20)).await;
    
    utils::run("kubectl", &[
        "wait", "--namespace", "ingress-nginx",
        "--for=condition=ready", "pod",
        "--selector=app.kubernetes.io/component=controller",
        "--timeout=300s"
    ])?;
    
    println!("✅ NGINX Ingress installed via Helm");
    Ok(())
}

async fn install_monitoring_stack_helm() -> Result<()> {
    println!("\n📊 Installing kube-prometheus-stack via Helm...");
    
    utils::run("kubectl", &["create", "namespace", "monitoring"])?;
    
    let values_file = get_values_file("kube-prometheus-stack");
    
    utils::run("helm", &[
        "install", "kube-prometheus-stack",
        "prometheus-community/kube-prometheus-stack",
        "--namespace", "monitoring",
        "--values", &values_file,
        "--version", "54.2.2",
    ])?;
    
    sleep(Duration::from_secs(30)).await;
    
    println!("✅ Monitoring stack installed via Helm");
    Ok(())
}

async fn install_logging_stack_helm() -> Result<()> {
    println!("\n📝 Installing EFK stack via Helm...");
    
    utils::run("kubectl", &["create", "namespace", "logging"])?;
    
    // Install Elasticsearch
    println!("   Installing Elasticsearch...");
    let es_values = get_values_file("elasticsearch");
    
    utils::run("helm", &[
        "install", "elasticsearch", "elastic/elasticsearch",
        "--namespace", "logging",
        "--values", &es_values,
        "--version", "8.5.1",
    ])?;
    
    sleep(Duration::from_secs(30)).await;
    
    // Install Fluentd
    println!("   Installing Fluentd...");
    let fluentd_values = get_values_file("fluentd");
    
    utils::run("helm", &[
        "install", "fluentd", "fluent/fluentd",
        "--namespace", "logging",
        "--values", &fluentd_values,
    ])?;
    
    // Install Kibana
    println!("   Installing Kibana...");
    let kibana_values = get_values_file("kibana");
    
    utils::run("helm", &[
        "install", "kibana", "elastic/kibana",
        "--namespace", "logging",
        "--values", &kibana_values,
        "--version", "8.5.1",
    ])?;
    
    println!("✅ EFK stack installed via Helm");
    Ok(())
}

async fn install_argocd_helm() -> Result<()> {
    println!("\n🔄 Installing ArgoCD via Helm...");
    
    utils::run("kubectl", &["create", "namespace", "argocd"])?;
    
    let values_file = get_values_file("argocd");
    
    utils::run("helm", &[
        "install", "argocd", "argo/argo-cd",
        "--namespace", "argocd",
        "--values", &values_file,
        "--version", "5.51.6",
    ])?;
    
    sleep(Duration::from_secs(30)).await;
    
    utils::run("kubectl", &[
        "wait", "--namespace", "argocd",
        "--for=condition=ready", "pod",
        "--selector=app.kubernetes.io/name=argocd-server",
        "--timeout=300s"
    ])?;
    
    println!("✅ ArgoCD installed via Helm");
    Ok(())
}

async fn deploy_sample_app_helm() -> Result<()> {
    println!("\n🚀 Deploying sample NGINX app...");
    
    let chart_dir = format!("{}/charts/sample-nginx", HELM_VALUES_DIR);
    let values_file = get_values_file("sample-nginx");
    
    utils::run("helm", &[
        "install", "sample-nginx", &chart_dir,
        "--namespace", "production",
        "--values", &values_file,
    ])?;
    
    sleep(Duration::from_secs(10)).await;
    println!("✅ Sample NGINX app deployed via Helm");
    Ok(())
}

fn setup_namespaces() -> Result<()> {
    println!("\n📂 Creating application namespaces...");
    
    let namespaces = r#"
apiVersion: v1
kind: Namespace
metadata:
  name: production
  labels:
    environment: production
---
apiVersion: v1
kind: Namespace
metadata:
  name: staging
  labels:
    environment: staging
---
apiVersion: v1
kind: Namespace
metadata:
  name: development
  labels:
    environment: development
"#;
    
    utils::apply_manifest(namespaces)?;
    println!("✅ Namespaces created");
    Ok(())
}

fn setup_network_policies() -> Result<()> {
    println!("\n🔒 Setting up network policies...");
    
    let policies = r#"
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: default-deny-ingress
  namespace: production
spec:
  podSelector: {}
  policyTypes:
  - Ingress
---
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: allow-same-namespace
  namespace: production
spec:
  podSelector: {}
  policyTypes:
  - Ingress
  ingress:
  - from:
    - podSelector: {}
"#;
    
    utils::apply_manifest(policies)?;
    println!("✅ Network policies applied");
    Ok(())
}

fn setup_resource_quotas() -> Result<()> {
    println!("\n💾 Setting up resource quotas...");
    
    let quotas = r#"
apiVersion: v1
kind: ResourceQuota
metadata:
  name: compute-quota
  namespace: production
spec:
  hard:
    requests.cpu: "10"
    requests.memory: 20Gi
    limits.cpu: "20"
    limits.memory: 40Gi
---
apiVersion: v1
kind: LimitRange
metadata:
  name: resource-limits
  namespace: production
spec:
  limits:
  - max:
      cpu: "2"
      memory: 4Gi
    min:
      cpu: 100m
      memory: 128Mi
    default:
      cpu: 500m
      memory: 512Mi
    defaultRequest:
      cpu: 250m
      memory: 256Mi
    type: Container
"#;
    
    utils::apply_manifest(quotas)?;
    println!("✅ Resource quotas set");
    Ok(())
}

fn print_access_info(cluster_name: &str) {
    let separator = "=".repeat(60);
    println!("\n{}", separator);
    println!("🎯 Access Information for '{}':", cluster_name);
    println!("{}", separator);
    println!("\n📊 Monitoring:");
    println!("  Prometheus: http://localhost:9090");
    println!("  Grafana:    http://localhost:3000 (admin/admin)");
    println!("\n📝 Logging:");
    println!("  Kibana:     http://localhost:5601");
    println!("\n🔄 GitOps:");
    println!("  ArgoCD:     http://localhost:8080 (admin)");
    println!("  Password:   kubectl -n argocd get secret argocd-initial-admin-secret -o jsonpath=\"{{.data.password}}\" | base64 -d");
    println!("\n🌐 Sample App:");
    println!("  Add to /etc/hosts: 127.0.0.1 nginx.local");
    println!("  Then visit: http://nginx.local");
    println!("\n📦 Installed Helm Releases:");
    println!("  helm list -A");
    println!("\n🔍 Useful Commands:");
    println!("  kubectl get pods -A");
    println!("  kubectl config use-context k3d-{}", cluster_name);
    println!("  helm list -n monitoring");
    println!("  k3d cluster delete {}", cluster_name);
    println!("\n{}", separator);
}