// prod_cluster.rs
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::sleep;
use std::fs;
use std::path::Path;
use std::process::Command;
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

    // Check if helm is installed
    let helm_available = check_helm_installed();
    if !helm_available {
        println!("\n⚠️  Warning: Helm is not installed!");
        println!("   Install with: brew install helm (macOS) or curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash");
        println!("   Continuing with cluster creation only...\n");
    }

    // Verify helm values directory exists if helm is available
    if helm_available {
        ensure_helm_values_dir()?;
    }

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
    
    if !helm_available {
        println!("\n⚠️  Skipping Helm installations (Helm not available)");
        setup_namespaces()?;
        println!("\n✅ Basic cluster '{}' is ready!", config.name);
        print_basic_access_info(&config.name);
        return Ok(());
    }
    
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

fn check_helm_installed() -> bool {
    Command::new("helm")
        .arg("version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn ensure_helm_values_dir() -> Result<()> {
    let helm_dir = Path::new(HELM_VALUES_DIR);
    if !helm_dir.exists() {
        println!("⚠️  Warning: Helm values directory not found at '{}'", HELM_VALUES_DIR);
        println!("   Creating directory structure...");
        fs::create_dir_all(helm_dir)
            .context(format!("Failed to create directory: {}", HELM_VALUES_DIR))?;
        
        // Create subdirectories
        fs::create_dir_all(format!("{}/charts/sample-nginx/templates", HELM_VALUES_DIR))?;
        fs::create_dir_all(format!("{}/manifests", HELM_VALUES_DIR))?;
        
        // Create default values files
        create_default_values_files()?;
        
        println!("   ✅ Created {} with default values files", HELM_VALUES_DIR);
    }
    Ok(())
}

fn create_default_values_files() -> Result<()> {
    // cert-manager values
    let cert_manager_values = include_str!("../helm-values-templates/cert-manager.yaml");
    fs::write(get_values_file("cert-manager"), cert_manager_values)?;
    
    // ingress-nginx values
    let ingress_values = include_str!("../helm-values-templates/ingress-nginx.yaml");
    fs::write(get_values_file("ingress-nginx"), ingress_values)?;
    
    // kube-prometheus-stack values
    let prometheus_values = include_str!("../helm-values-templates/kube-prometheus-stack.yaml");
    fs::write(get_values_file("kube-prometheus-stack"), prometheus_values)?;
    
    // elasticsearch values
    let es_values = include_str!("../helm-values-templates/elasticsearch.yaml");
    fs::write(get_values_file("elasticsearch"), es_values)?;
    
    // fluentd values
    let fluentd_values = include_str!("../helm-values-templates/fluentd.yaml");
    fs::write(get_values_file("fluentd"), fluentd_values)?;
    
    // kibana values
    let kibana_values = include_str!("../helm-values-templates/kibana.yaml");
    fs::write(get_values_file("kibana"), kibana_values)?;
    
    // argocd values
    let argocd_values = include_str!("../helm-values-templates/argocd.yaml");
    fs::write(get_values_file("argocd"), argocd_values)?;
    
    // sample-nginx chart
    create_sample_nginx_chart()?;
    
    // kubernetes manifests
    let issuer_manifest = include_str!("../helm-values-templates/manifests/cert-issuer.yaml");
    fs::write(format!("{}/manifests/cert-issuer.yaml", HELM_VALUES_DIR), issuer_manifest)?;
    
    let namespaces_manifest = include_str!("../helm-values-templates/manifests/namespaces.yaml");
    fs::write(format!("{}/manifests/namespaces.yaml", HELM_VALUES_DIR), namespaces_manifest)?;
    
    let network_policies_manifest = include_str!("../helm-values-templates/manifests/network-policies.yaml");
    fs::write(format!("{}/manifests/network-policies.yaml", HELM_VALUES_DIR), network_policies_manifest)?;
    
    let quotas_manifest = include_str!("../helm-values-templates/manifests/resource-quotas.yaml");
    fs::write(format!("{}/manifests/resource-quotas.yaml", HELM_VALUES_DIR), quotas_manifest)?;
    
    Ok(())
}

fn create_sample_nginx_chart() -> Result<()> {
    let chart_dir = format!("{}/charts/sample-nginx", HELM_VALUES_DIR);
    
    // Chart.yaml
    let chart_yaml = include_str!("../helm-values-templates/charts/sample-nginx/Chart.yaml");
    fs::write(format!("{}/Chart.yaml", chart_dir), chart_yaml)?;
    
    // values.yaml
    let values_yaml = include_str!("../helm-values-templates/charts/sample-nginx/values.yaml");
    fs::write(format!("{}/values.yaml", chart_dir), values_yaml)?;
    
    // templates/deployment.yaml
    let deployment_yaml = include_str!("../helm-values-templates/charts/sample-nginx/templates/deployment.yaml");
    fs::write(format!("{}/templates/deployment.yaml", chart_dir), deployment_yaml)?;
    
    // templates/service.yaml
    let service_yaml = include_str!("../helm-values-templates/charts/sample-nginx/templates/service.yaml");
    fs::write(format!("{}/templates/service.yaml", chart_dir), service_yaml)?;
    
    // templates/ingress.yaml
    let ingress_yaml = include_str!("../helm-values-templates/charts/sample-nginx/templates/ingress.yaml");
    fs::write(format!("{}/templates/ingress.yaml", chart_dir), ingress_yaml)?;
    
    Ok(())
}

fn get_values_file(component: &str) -> String {
    format!("{}/{}.yaml", HELM_VALUES_DIR, component)
}

fn get_manifest_file(name: &str) -> String {
    format!("{}/manifests/{}.yaml", HELM_VALUES_DIR, name)
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
    let values_exists = Path::new(&values_file).exists();
    
    let mut args = vec![
        "install", "cert-manager", "jetstack/cert-manager",
        "--namespace", "cert-manager",
        "--version", "v1.13.2",
        "--set", "installCRDs=true",
    ];
    
    if values_exists {
        args.push("--values");
        args.push(&values_file);
    } else {
        println!("   ℹ️  Using default values (no custom values file found)");
    }
    
    utils::run("helm", &args)?;
    
    sleep(Duration::from_secs(30)).await;
    
    utils::run("kubectl", &[
        "wait", "--for=condition=ready", "pod",
        "-l", "app.kubernetes.io/instance=cert-manager",
        "-n", "cert-manager",
        "--timeout=300s"
    ])?;
    
    // Apply cert issuer from manifest file
    let issuer_manifest = get_manifest_file("cert-issuer");
    if Path::new(&issuer_manifest).exists() {
        utils::run("kubectl", &["apply", "-f", &issuer_manifest])?;
    } else {
        // Fallback to inline manifest
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
    }
    
    println!("✅ Cert-manager installed via Helm");
    Ok(())
}

async fn install_ingress_controller_helm() -> Result<()> {
    println!("\n🌐 Installing NGINX Ingress via Helm...");
    
    utils::run("kubectl", &["create", "namespace", "ingress-nginx"])?;
    
    let values_file = get_values_file("ingress-nginx");
    let values_exists = Path::new(&values_file).exists();
    
    let mut args = vec![
        "install", "ingress-nginx", "ingress-nginx/ingress-nginx",
        "--namespace", "ingress-nginx",
    ];
    
    if values_exists {
        args.push("--values");
        args.push(&values_file);
    } else {
        println!("   ℹ️  Using default values");
        args.push("--set");
        args.push("controller.hostPort.enabled=true");
        args.push("--set");
        args.push("controller.service.type=NodePort");
    }
    
    utils::run("helm", &args)?;
    
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
    let values_exists = Path::new(&values_file).exists();
    
    let mut args = vec![
        "install", "kube-prometheus-stack",
        "prometheus-community/kube-prometheus-stack",
        "--namespace", "monitoring",
        "--version", "54.2.2",
    ];
    
    if values_exists {
        args.push("--values");
        args.push(&values_file);
    } else {
        println!("   ℹ️  Using default values");
    }
    
    utils::run("helm", &args)?;
    
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
    let es_values_exists = Path::new(&es_values).exists();
    
    let mut es_args = vec![
        "install", "elasticsearch", "elastic/elasticsearch",
        "--namespace", "logging",
        "--version", "8.5.1",
    ];
    
    if es_values_exists {
        es_args.push("--values");
        es_args.push(&es_values);
    } else {
        es_args.push("--set");
        es_args.push("replicas=1");
        es_args.push("--set");
        es_args.push("minimumMasterNodes=1");
    }
    
    utils::run("helm", &es_args)?;
    
    sleep(Duration::from_secs(30)).await;
    
    // Install Fluentd
    println!("   Installing Fluentd...");
    let fluentd_values = get_values_file("fluentd");
    let fluentd_values_exists = Path::new(&fluentd_values).exists();
    
    let mut fluentd_args = vec![
        "install", "fluentd", "fluent/fluentd",
        "--namespace", "logging",
    ];
    
    if fluentd_values_exists {
        fluentd_args.push("--values");
        fluentd_args.push(&fluentd_values);
    }
    
    utils::run("helm", &fluentd_args)?;
    
    // Install Kibana
    println!("   Installing Kibana...");
    let kibana_values = get_values_file("kibana");
    let kibana_values_exists = Path::new(&kibana_values).exists();
    
    let mut kibana_args = vec![
        "install", "kibana", "elastic/kibana",
        "--namespace", "logging",
        "--version", "8.5.1",
    ];
    
    if kibana_values_exists {
        kibana_args.push("--values");
        kibana_args.push(&kibana_values);
    }
    
    utils::run("helm", &kibana_args)?;
    
    println!("✅ EFK stack installed via Helm");
    Ok(())
}

async fn install_argocd_helm() -> Result<()> {
    println!("\n🔄 Installing ArgoCD via Helm...");
    
    utils::run("kubectl", &["create", "namespace", "argocd"])?;
    
    let values_file = get_values_file("argocd");
    let values_exists = Path::new(&values_file).exists();
    
    let mut args = vec![
        "install", "argocd", "argo/argo-cd",
        "--namespace", "argocd",
        "--version", "5.51.6",
    ];
    
    if values_exists {
        args.push("--values");
        args.push(&values_file);
    } else {
        println!("   ℹ️  Using default values");
    }
    
    utils::run("helm", &args)?;
    
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
    
    // Check if custom chart exists
    if Path::new(&chart_dir).exists() && Path::new(&format!("{}/Chart.yaml", chart_dir)).exists() {
        utils::run("helm", &[
            "install", "sample-nginx", &chart_dir,
            "--namespace", "production",
            "--create-namespace",
        ])?;
    } else {
        println!("   ℹ️  Custom chart not found, deploying basic NGINX with kubectl...");
        deploy_basic_nginx()?;
    }
    
    sleep(Duration::from_secs(10)).await;
    println!("✅ Sample NGINX app deployed");
    Ok(())
}

fn deploy_basic_nginx() -> Result<()> {
    let nginx_manifest = r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: sample-nginx
  namespace: production
spec:
  replicas: 2
  selector:
    matchLabels:
      app: nginx
  template:
    metadata:
      labels:
        app: nginx
    spec:
      containers:
      - name: nginx
        image: nginx:latest
        ports:
        - containerPort: 80
---
apiVersion: v1
kind: Service
metadata:
  name: sample-nginx
  namespace: production
spec:
  selector:
    app: nginx
  ports:
  - port: 80
    targetPort: 80
"#;
    
    utils::apply_manifest(nginx_manifest)?;
    Ok(())
}

fn setup_namespaces() -> Result<()> {
    println!("\n📂 Creating application namespaces...");
    
    let manifest_file = get_manifest_file("namespaces");
    if Path::new(&manifest_file).exists() {
        utils::run("kubectl", &["apply", "-f", &manifest_file])?;
    } else {
        // Fallback to inline manifest
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
    }
    
    println!("✅ Namespaces created");
    Ok(())
}

fn setup_network_policies() -> Result<()> {
    println!("\n🔒 Setting up network policies...");
    
    let manifest_file = get_manifest_file("network-policies");
    if Path::new(&manifest_file).exists() {
        utils::run("kubectl", &["apply", "-f", &manifest_file])?;
    } else {
        // Fallback to inline manifest
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
    }
    
    println!("✅ Network policies applied");
    Ok(())
}

fn setup_resource_quotas() -> Result<()> {
    println!("\n💾 Setting up resource quotas...");
    
    let manifest_file = get_manifest_file("resource-quotas");
    if Path::new(&manifest_file).exists() {
        utils::run("kubectl", &["apply", "-f", &manifest_file])?;
    } else {
        // Fallback to inline manifest
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
    }
    
    println!("✅ Resource quotas set");
    Ok(())
}

fn print_basic_access_info(cluster_name: &str) {
    let separator = "=".repeat(60);
    println!("\n{}", separator);
    println!("🎯 Access Information for '{}':", cluster_name);
    println!("{}", separator);
    println!("\n📦 Basic cluster created (Helm components not installed)");
    println!("\n🔍 Useful Commands:");
    println!("  kubectl get pods -A");
    println!("  kubectl config use-context k3d-{}", cluster_name);
    println!("  k3d cluster delete {}", cluster_name);
    println!("\n💡 To install Helm components, first install Helm:");
    println!("  macOS:   brew install helm");
    println!("  Linux:   curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash");
    println!("  Windows: choco install kubernetes-helm");
    println!("\n{}", separator);
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
    println!("\n📁 Configuration Files:");
    println!("  Helm values: {}", HELM_VALUES_DIR);
    println!("  Edit values: vim {}/cert-manager.yaml", HELM_VALUES_DIR);
    println!("\n{}", separator);
}