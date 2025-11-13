#!/usr/bin/env bash
set -euo pipefail

echo "🧰 Setting up local Kubernetes + k3s dev environment on Pop!_OS..."

# --- Update system packages ---
sudo apt update -y
sudo apt install -y apt-transport-https ca-certificates curl gnupg lsb-release software-properties-common

# --- Install Docker (required for k3d) ---
if ! command -v docker &> /dev/null; then
  echo "🐳 Installing Docker..."
  curl -fsSL https://get.docker.com | sudo bash
  sudo usermod -aG docker $USER
  echo "✅ Docker installed (you may need to log out and back in)."
else
  echo "✅ Docker already installed."
fi

# --- Install containerd (runtime used by k3s) ---
if ! command -v containerd &> /dev/null; then
  echo "📦 Installing containerd..."
  sudo apt install -y containerd.io
  sudo systemctl enable containerd
  sudo systemctl start containerd
  echo "✅ containerd installed and running."
else
  echo "✅ containerd already installed."
fi

# --- Install kubectl ---
if ! command -v kubectl &> /dev/null; then
  echo "⚙️ Installing kubectl..."
  sudo mkdir -p /etc/apt/keyrings
  curl -fsSL https://pkgs.k8s.io/core:/stable:/v1.30/deb/Release.key | \
    sudo gpg --dearmor -o /etc/apt/keyrings/kubernetes-apt-keyring.gpg

  echo "deb [signed-by=/etc/apt/keyrings/kubernetes-apt-keyring.gpg] \
  https://pkgs.k8s.io/core:/stable:/v1.30/deb/ /" | \
  sudo tee /etc/apt/sources.list.d/kubernetes.list

  sudo apt update -y
  sudo apt install -y kubectl
  echo "✅ kubectl installed successfully."
else
  echo "✅ kubectl already installed."
fi


# --- Install k3d ---
if ! command -v k3d &> /dev/null; then
  echo "🚀 Installing k3d..."
  curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
  echo "✅ k3d installed."
else
  echo "✅ k3d already installed."
fi

echo ""
echo "🎉 Environment setup complete!"
echo "   - Docker 🐳"
echo "   - containerd 📦"
echo "   - kubectl ⚙️"
echo "   - k3d 🚀"
echo ""
echo "You can now run your Rust program to create your 3-node k3s cluster."
echo "If Docker was newly installed, log out/in or run: newgrp docker"
