#!/usr/bin/env bash
# https://docs.docker.com/engine/install/ubuntu/
set -euo pipefail
if [[ "$(id -u)" != 0 ]]; then
    echo "Run: sudo bash scripts/install-docker-ubuntu.sh" >&2
    exit 1
fi
source /etc/os-release
if [[ "${ID:-}" != ubuntu ]]; then
    echo "This script supports Ubuntu only." >&2
    exit 1
fi
case "${UBUNTU_CODENAME:-${VERSION_CODENAME:-}}" in
    noble|jammy|resolute|stonking) ;;
    *) echo "Review Docker's supported Ubuntu versions before installing." >&2; exit 1 ;;
esac
for package in docker.io docker-compose docker-compose-v2 docker-doc docker-buildx podman-docker containerd runc; do
    if [[ "$(dpkg-query -W -f='${Status}' "$package" 2>/dev/null || true)" == "install ok installed" ]]; then
        echo "Conflicting package installed: $package. Resolve it before running this script." >&2
        exit 1
    fi
done
apt-get update
apt-get install -y ca-certificates curl
install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
chmod a+r /etc/apt/keyrings/docker.asc
docker_codename="${UBUNTU_CODENAME:-$VERSION_CODENAME}"
docker_arch="$(dpkg --print-architecture)"
cat > /etc/apt/sources.list.d/docker.sources <<EOF
Types: deb
URIs: https://download.docker.com/linux/ubuntu
Suites: $docker_codename
Components: stable
Architectures: $docker_arch
Signed-By: /etc/apt/keyrings/docker.asc
EOF
apt-get update
apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
systemctl enable --now docker
docker version
docker compose version
docker run --rm hello-world
echo "Docker is ready. Use sudo docker ...; user group membership was not changed."
