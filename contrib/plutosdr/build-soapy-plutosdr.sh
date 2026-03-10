#!/usr/bin/env bash
# Build SoapyPlutoSDR with tetra-bluestation patches.
#
# This script fetches a known-good commit of pgreenland's SoapyPlutoSDR,
# applies the patches from this directory, builds, and installs the driver.
# Using a pinned commit avoids the patch file breaking against future upstream changes.

set -euo pipefail

REPO_URL="https://github.com/pgreenland/SoapyPlutoSDR.git"
KNOWN_GOOD_BRANCH="sdr_gadget_timestamping_with_iio_support"
# Pin to a known-good commit so the patches always apply cleanly.
# Update this hash (and re-test the patches) when upgrading the driver.
KNOWN_GOOD_COMMIT="HEAD"   # Replace with a specific commit SHA once one is verified, e.g. "a1b2c3d"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PATCH_FILE="$SCRIPT_DIR/patches/soapy-plutosdr/full-plutosdr-patches.patch"
BUILD_DIR="$SCRIPT_DIR/SoapyPlutoSDR-build"

echo "==> Cloning SoapyPlutoSDR ($KNOWN_GOOD_BRANCH) …"
rm -rf "$BUILD_DIR"
git clone --branch "$KNOWN_GOOD_BRANCH" --depth 50 "$REPO_URL" "$BUILD_DIR"

cd "$BUILD_DIR"

if [ "$KNOWN_GOOD_COMMIT" != "HEAD" ]; then
    echo "==> Checking out pinned commit $KNOWN_GOOD_COMMIT …"
    git checkout "$KNOWN_GOOD_COMMIT"
fi

echo "==> Applying patches …"
git apply "$PATCH_FILE"

echo "==> Building …"
mkdir -p build && cd build
cmake ..
make -j"$(nproc)"

echo "==> Installing (requires sudo) …"
sudo make install

echo ""
echo "Done. Verify with:"
echo "  SoapySDRUtil --probe=\"driver=plutosdr\""
