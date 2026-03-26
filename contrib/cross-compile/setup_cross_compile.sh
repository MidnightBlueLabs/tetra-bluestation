#!/usr/bin/env bash
set -euo pipefail

# setup_cross_compile.sh
#
# Installs:
# - Docker Engine / docker.io
# - docker group membership for the invoking user
# - Rust `cross`
# - Adds cargo bin directory to $PATH
#
# Assumptions:
# - Rust + cargo + rustup are already installed
#
# Usage:
#   ./setup_cross_compile.sh
#
# Notes:
# - Must not be run as root.
# - Group membership changes require a new login session.
# - Path change needs new shell.

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[-] error: required command not found: $1" >&2
    exit 1
  fi
}

ensure_not_root() {
  if [[ "${EUID}" -eq 0 ]]; then
    echo "[-] error: do not run this script as root" >&2
    echo "    Run it as your normal user. The script will use sudo when needed." >&2
    exit 1
  fi
}

confirm_plan() {
  cat <<'EOF'
[i] This script will perform the following actions:

    1. Install Docker via apt if it is not already installed.
    2. Enable and start the Docker service.
    3. Add your user to the docker group if needed.
    4. Install the Rust `cross` tool if it is not already installed.
    5. Ensure $HOME/.cargo/bin is added to your PATH in ~/.bashrc if needed.

    This script may call sudo for system changes.
    It must be run as a normal user, not as root.

Proceed? [y/N]
EOF

  read -r reply
  case "${reply}" in
    y|Y|yes|YES)
      ;;
    *)
      echo "[i] Aborted."
      exit 0
      ;;
  esac
}

need_root() {
  if [[ "${EUID}" -ne 0 ]]; then
    require_cmd sudo
    sudo "$@"
  else
    "$@"
  fi
}

detect_user() {
  if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
    printf '%s\n' "${SUDO_USER}"
  else
    id -un
  fi
}

user_home() {
  local target_user
  target_user="$(detect_user)"
  getent passwd "${target_user}" | cut -d: -f6
}

install_docker_debian() {
  if command -v docker >/dev/null 2>&1; then
    echo "[+] docker already installed"
    return
  fi

  echo "[i] installing docker"
  need_root apt-get update
  need_root apt-get install -y docker.io
  need_root systemctl enable --now docker
}

ensure_docker_group_membership() {
  local target_user
  target_user="$(detect_user)"

  if id -nG "${target_user}" | tr ' ' '\n' | grep -qx docker; then
    echo "[+] user '${target_user}' is already in docker group"
    return
  fi

  echo "[i] adding user '${target_user}' to docker group"
  need_root usermod -aG docker "${target_user}"

  cat <<EOF

[+] docker group membership was updated for user '${target_user}'.
You must log out and log back in before you can proceed.
EOF
}

install_cross() {
  require_cmd cargo
  require_cmd rustup

  if command -v cross >/dev/null 2>&1; then
    echo "[+] cross already installed: $(command -v cross)"
    return
  fi

  echo "[i] installing cross"
  cargo install cross --locked
}

ensure_cargo_bin_in_bashrc_if_needed() {
  local target_user home_dir bashrc cargo_bin line
  target_user="$(detect_user)"
  home_dir="$(user_home)"
  bashrc="${home_dir}/.bashrc"
  cargo_bin="${home_dir}/.cargo/bin"
  line='export PATH="$HOME/.cargo/bin:$PATH"'

  if command -v cross >/dev/null 2>&1; then
    echo "[+] cross is already available in PATH: $(command -v cross)"
    return
  fi

  echo "[!] cross is installed but not currently available in PATH"

  mkdir -p "${cargo_bin}"

  if [[ ! -f "${bashrc}" ]]; then
    touch "${bashrc}"
    chown "${target_user}:${target_user}" "${bashrc}" 2>/dev/null || true
  fi

  if ! grep -Fqx "${line}" "${bashrc}"; then
    printf '\n%s\n' "${line}" >> "${bashrc}"
    chown "${target_user}:${target_user}" "${bashrc}" 2>/dev/null || true
    echo "[+] added Cargo bin directory to ${bashrc}"
  else
    echo "[+] Cargo bin PATH line already present in ${bashrc}"
  fi

  cat <<EOF

[i] PATH was updated in ${bashrc}, but the current shell will not pick it up automatically.
    Log out and log back in, then verify:
      command -v cross
      cross --version
EOF
}

main() {
  confirm_plan
  ensure_not_root
  require_cmd apt-get
  require_cmd getent

  install_docker_debian
  ensure_docker_group_membership
  install_cross
  ensure_cargo_bin_in_bashrc_if_needed

  cat <<'EOF'

[+] Setup complete.
    Verify these two commands work:
      docker --version
      cross --version

    Then run cross compilation:
      cross build --release --target aarch64-unknown-linux-gnu
EOF
}

main "$@"