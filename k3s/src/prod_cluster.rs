// prod_cluster.rs
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::sleep;
use std::fs;
use crate::utils;  // Import the utils module!

// Make these PUBLIC so main.rs can use them!
pub struct ProdClusterConfig {
    pub name: String,
    pub servers: u8,
    pub agents: u8,
    pub install_monitoring: bool,
    pub install_logging: bool,
    pub install_argocd: bool,
}

pub async fn create_prod_cluster(config: ProdClusterConfig) -> Result<()> {
    println!("🚀 Building production-like k3d cluster: {}", config.name);
    println!("📋 Configuration:");
    println!("   Control Plane Nodes: {}", config.servers);
    println!("   Worker Nodes: {}", config.agents);
    println!("   Monitoring: {}", if config.install_monitoring { "✓" } else { "✗" });
    println!("   Logging: {}", if config.install_logging { "✓" } else { "✗" });
    println!("   ArgoCD: {}", if config.install_argocd { "✓" } else { "✗" });

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
    
    println!("\n📦 Installing production components...");
    
    // Install core infrastructure
    install_cert_manager().await?;
    install_ingress_controller().await?;
    
    if config.install_monitoring {
        install_monitoring_stack().await?;
    }
    
    if config.install_logging {
        install_logging_stack().await?;
    }
    
    if config.install_argocd {
        install_argocd().await?;
    }
    
    // Setup namespaces and policies
    setup_namespaces()?;
    setup_network_policies()?;
    setup_resource_quotas()?;
    setup_rbac()?;
    
    // Deploy sample applications
    deploy_sample_apps().await?;

    println!("\n🎉 Production-like cluster '{}' is ready!", config.name);
    print_access_info(&config.name);
    
    Ok(())
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

async fn install_cert_manager() -> Result<()> {
    println!("\n🔐 Installing cert-manager...");
    
    utils::run("kubectl", &[
        "apply", "-f",
        "https://github.com/cert-manager/cert-manager/releases/download/v1.13.2/cert-manager.yaml"
    ])?;
    
    sleep(Duration::from_secs(30)).await;
    
    utils::run("kubectl", &[
        "wait", "--for=condition=ready", "pod",
        "-l", "app.kubernetes.io/instance=cert-manager",
        "-n", "cert-manager",
        "--timeout=300s"
    ])?;
    
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
    println!("✅ Cert-manager installed with local CA");
    Ok(())
}

async fn install_ingress_controller() -> Result<()> {
    println!("\n🌐 Installing NGINX Ingress Controller...");
    
    utils::run("kubectl", &[
        "apply", "-f",
        "https://raw.githubusercontent.com/kubernetes/ingress-nginx/controller-v1.9.4/deploy/static/provider/cloud/deploy.yaml"
    ])?;
    
    sleep(Duration::from_secs(20)).await;
    
    utils::run("kubectl", &[
        "wait", "--namespace", "ingress-nginx",
        "--for=condition=ready", "pod",
        "--selector=app.kubernetes.io/component=controller",
        "--timeout=300s"
    ])?;
    
    println!("✅ Ingress controller ready");
    Ok(())
}

async fn install_monitoring_stack() -> Result<()> {
    println!("\n📊 Installing Prometheus + Grafana...");
    
    // Keep your full monitoring manifest here
    let monitoring = r#"
apiVersion: v1
kind: Namespace
metadata:
  name: monitoring
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: prometheus
  namespace: monitoring
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: prometheus
rules:
- apiGroups: [""]
  resources:
  - nodes
  - nodes/proxy
  - services
  - endpoints
  - pods
  verbs: ["get", "list", "watch"]
- apiGroups:
  - extensions
  resources:
  - ingresses
  verbs: ["get", "list", "watch"]
- nonResourceURLs: ["/metrics"]
  verbs: ["get"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: prometheus
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: prometheus
subjects:
- kind: ServiceAccount
  name: prometheus
  namespace: monitoring
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-config
  namespace: monitoring
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
      evaluation_interval: 15s
    scrape_configs:
      - job_name: 'kubernetes-apiservers'
        kubernetes_sd_configs:
        - role: endpoints
        scheme: https
        tls_config:
          ca_file: /var/run/secrets/kubernetes.io/serviceaccount/ca.crt
        bearer_token_file: /var/run/secrets/kubernetes.io/serviceaccount/token
        relabel_configs:
        - source_labels: [__meta_kubernetes_namespace, __meta_kubernetes_service_name, __meta_kubernetes_endpoint_port_name]
          action: keep
          regex: default;kubernetes;https
      - job_name: 'kubernetes-nodes'
        kubernetes_sd_configs:
        - role: node
        scheme: https
        tls_config:
          ca_file: /var/run/secrets/kubernetes.io/serviceaccount/ca.crt
        bearer_token_file: /var/run/secrets/kubernetes.io/serviceaccount/token
      - job_name: 'kubernetes-pods'
        kubernetes_sd_configs:
        - role: pod
        relabel_configs:
        - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
          action: keep
          regex: true
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: prometheus
  namespace: monitoring
spec:
  replicas: 1
  selector:
    matchLabels:
      app: prometheus
  template:
    metadata:
      labels:
        app: prometheus
    spec:
      serviceAccountName: prometheus
      containers:
      - name: prometheus
        image: prom/prometheus:v2.48.0
        args:
          - '--config.file=/etc/prometheus/prometheus.yml'
          - '--storage.tsdb.path=/prometheus/'
          - '--web.console.libraries=/etc/prometheus/console_libraries'
          - '--web.console.templates=/etc/prometheus/consoles'
          - '--web.enable-lifecycle'
        ports:
        - containerPort: 9090
        volumeMounts:
        - name: prometheus-config
          mountPath: /etc/prometheus/
        - name: prometheus-storage
          mountPath: /prometheus/
      volumes:
      - name: prometheus-config
        configMap:
          name: prometheus-config
      - name: prometheus-storage
        emptyDir: {}
---
apiVersion: v1
kind: Service
metadata:
  name: prometheus
  namespace: monitoring
spec:
  selector:
    app: prometheus
  ports:
  - port: 9090
    targetPort: 9090
  type: LoadBalancer
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: grafana
  namespace: monitoring
spec:
  replicas: 1
  selector:
    matchLabels:
      app: grafana
  template:
    metadata:
      labels:
        app: grafana
    spec:
      containers:
      - name: grafana
        image: grafana/grafana:10.2.2
        ports:
        - containerPort: 3000
        env:
        - name: GF_SECURITY_ADMIN_PASSWORD
          value: "admin"
        - name: GF_SERVER_ROOT_URL
          value: "http://localhost:3000"
        volumeMounts:
        - name: grafana-storage
          mountPath: /var/lib/grafana
      volumes:
      - name: grafana-storage
        emptyDir: {}
---
apiVersion: v1
kind: Service
metadata:
  name: grafana
  namespace: monitoring
spec:
  selector:
    app: grafana
  ports:
  - port: 3000
    targetPort: 3000
  type: LoadBalancer
"#;
    
    utils::apply_manifest(monitoring)?;
    
    sleep(Duration::from_secs(15)).await;
    println!("✅ Monitoring stack installed");
    Ok(())
}

async fn install_logging_stack() -> Result<()> {
    println!("\n📝 Installing EFK (Elasticsearch, Fluentd, Kibana)...");
    
    // Keep your full logging manifest
    let logging = r#"
apiVersion: v1
kind: Namespace
metadata:
  name: logging
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: elasticsearch
  namespace: logging
spec:
  serviceName: elasticsearch
  replicas: 1
  selector:
    matchLabels:
      app: elasticsearch
  template:
    metadata:
      labels:
        app: elasticsearch
    spec:
      containers:
      - name: elasticsearch
        image: docker.elastic.co/elasticsearch/elasticsearch:8.11.1
        env:
        - name: discovery.type
          value: single-node
        - name: ES_JAVA_OPTS
          value: "-Xms512m -Xmx512m"
        - name: xpack.security.enabled
          value: "false"
        ports:
        - containerPort: 9200
          name: rest
        - containerPort: 9300
          name: inter-node
---
apiVersion: v1
kind: Service
metadata:
  name: elasticsearch
  namespace: logging
spec:
  selector:
    app: elasticsearch
  ports:
  - port: 9200
    name: rest
  - port: 9300
    name: inter-node
---
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: fluentd
  namespace: logging
spec:
  selector:
    matchLabels:
      app: fluentd
  template:
    metadata:
      labels:
        app: fluentd
    spec:
      serviceAccountName: fluentd
      containers:
      - name: fluentd
        image: fluent/fluentd-kubernetes-daemonset:v1-debian-elasticsearch
        env:
        - name: FLUENT_ELASTICSEARCH_HOST
          value: "elasticsearch.logging.svc.cluster.local"
        - name: FLUENT_ELASTICSEARCH_PORT
          value: "9200"
        volumeMounts:
        - name: varlog
          mountPath: /var/log
        - name: varlibdockercontainers
          mountPath: /var/lib/docker/containers
          readOnly: true
      volumes:
      - name: varlog
        hostPath:
          path: /var/log
      - name: varlibdockercontainers
        hostPath:
          path: /var/lib/docker/containers
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: fluentd
  namespace: logging
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: fluentd
rules:
- apiGroups: [""]
  resources:
  - pods
  - namespaces
  verbs: ["get", "list", "watch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: fluentd
roleRef:
  kind: ClusterRole
  name: fluentd
  apiGroup: rbac.authorization.k8s.io
subjects:
- kind: ServiceAccount
  name: fluentd
  namespace: logging
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: kibana
  namespace: logging
spec:
  replicas: 1
  selector:
    matchLabels:
      app: kibana
  template:
    metadata:
      labels:
        app: kibana
    spec:
      containers:
      - name: kibana
        image: docker.elastic.co/kibana/kibana:8.11.1
        env:
        - name: ELASTICSEARCH_HOSTS
          value: "http://elasticsearch:9200"
        ports:
        - containerPort: 5601
---
apiVersion: v1
kind: Service
metadata:
  name: kibana
  namespace: logging
spec:
  selector:
    app: kibana
  ports:
  - port: 5601
  type: LoadBalancer
"#;
    
    utils::apply_manifest(logging)?;
    println!("✅ Logging stack installed");
    Ok(())
}

async fn install_argocd() -> Result<()> {
    println!("\n🔄 Installing ArgoCD...");
    
    utils::run("kubectl", &[
        "create", "namespace", "argocd"
    ])?;
    
    utils::run("kubectl", &[
        "apply", "-n", "argocd", "-f",
        "https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/install.yaml"
    ])?;
    
    sleep(Duration::from_secs(30)).await;
    
    utils::run("kubectl", &[
        "wait", "--namespace", "argocd",
        "--for=condition=ready", "pod",
        "--selector=app.kubernetes.io/name=argocd-server",
        "--timeout=300s"
    ])?;
    
    utils::run("kubectl", &[
        "patch", "svc", "argocd-server",
        "-n", "argocd",
        "-p", r#"{"spec": {"type": "LoadBalancer"}}"#
    ])?;
    
    println!("✅ ArgoCD installed");
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
---
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: allow-from-ingress
  namespace: production
spec:
  podSelector:
    matchLabels:
      app: web
  policyTypes:
  - Ingress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
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
    persistentvolumeclaims: "10"
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

fn setup_rbac() -> Result<()> {
    println!("\n👥 Setting up RBAC policies...");
    
    let rbac = r#"
apiVersion: v1
kind: ServiceAccount
metadata:
  name: developer
  namespace: development
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: developer-role
  namespace: development
rules:
- apiGroups: ["", "apps", "batch"]
  resources: ["*"]
  verbs: ["*"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: developer-binding
  namespace: development
subjects:
- kind: ServiceAccount
  name: developer
  namespace: development
roleRef:
  kind: Role
  name: developer-role
  apiGroup: rbac.authorization.k8s.io
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: readonly
  namespace: production
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: readonly-role
rules:
- apiGroups: [""]
  resources: ["*"]
  verbs: ["get", "list", "watch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: readonly-binding
subjects:
- kind: ServiceAccount
  name: readonly
  namespace: production
roleRef:
  kind: ClusterRole
  name: readonly-role
  apiGroup: rbac.authorization.k8s.io
"#;
    
    utils::apply_manifest(rbac)?;
    println!("✅ RBAC policies configured");
    Ok(())
}

async fn deploy_sample_apps() -> Result<()> {
    println!("\n🚀 Deploying sample applications...");
    
    let apps = r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nginx-app
  namespace: production
spec:
  replicas: 3
  selector:
    matchLabels:
      app: nginx
  template:
    metadata:
      labels:
        app: nginx
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9113"
    spec:
      containers:
      - name: nginx
        image: nginx:alpine
        ports:
        - containerPort: 80
        resources:
          requests:
            cpu: 100m
            memory: 128Mi
          limits:
            cpu: 200m
            memory: 256Mi
        livenessProbe:
          httpGet:
            path: /
            port: 80
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /
            port: 80
          initialDelaySeconds: 5
          periodSeconds: 5
      - name: nginx-exporter
        image: nginx/nginx-prometheus-exporter:0.11.0
        args:
          - '-nginx.scrape-uri=http://localhost/stub_status'
        ports:
        - containerPort: 9113
---
apiVersion: v1
kind: Service
metadata:
  name: nginx-service
  namespace: production
spec:
  selector:
    app: nginx
  ports:
  - port: 80
    targetPort: 80
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: nginx-ingress
  namespace: production
  annotations:
    cert-manager.io/cluster-issuer: "local-ca-issuer"
spec:
  ingressClassName: nginx
  tls:
  - hosts:
    - nginx.local
    secretName: nginx-tls
  rules:
  - host: nginx.local
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: nginx-service
            port:
              number: 80
---
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: nginx-hpa
  namespace: production
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: nginx-app
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
"#;
    
    utils::apply_manifest(apps)?;
    
    sleep(Duration::from_secs(10)).await;
    println!("✅ Sample apps deployed");
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
    println!("  ArgoCD:     http://localhost:8080");
    println!("  Password:   kubectl -n argocd get secret argocd-initial-admin-secret -o jsonpath=\"{{.data.password}}\" | base64 -d");
    println!("\n🌐 Sample App:");
    println!("  Add to /etc/hosts: 127.0.0.1 nginx.local");
    println!("  Then visit: http://nginx.local");
    println!("\n🔍 Useful Commands:");
    println!("  kubectl get pods -A");
    println!("  kubectl config use-context k3d-{}", cluster_name);
    println!("  k3d cluster delete {}", cluster_name);
    println!("\n{}", separator);
}