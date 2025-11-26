#!/bin/bash

set -e  # Exit on error

tarpaulin() {
  workspace_name=$1
  output_dir="target/tarpaulin-reports/$workspace_name-coverage"
  
  echo "Running tarpaulin for $workspace_name..."
  mkdir -p "$output_dir"
  
  # Check if this is a workspace or a single package
  if grep -q "^\[workspace\]" Cargo.toml 2>/dev/null; then
    echo "Detected workspace, using --workspace flag"
    cargo +nightly tarpaulin --verbose --all-features --workspace --timeout 120 --out Xml --output-dir "$output_dir"
  else
    echo "Detected single package, not using --workspace flag"
    cargo +nightly tarpaulin --verbose --all-features --timeout 120 --out Xml --output-dir "$output_dir"
  fi
  
  # Verify the output file was created
  if [ -f "$output_dir/cobertura.xml" ]; then
    echo "✅ Coverage report created at: $output_dir/cobertura.xml"
  else
    echo "❌ Error: cobertura.xml not found at $output_dir/cobertura.xml"
    exit 1
  fi
}

echo "Running coverage analysis for bitcoin_core_sv2 crate..."
echo "================================================="

# bitcoin_core_sv2 Coverage
cd bitcoin-core-sv2 || exit 1
tarpaulin "bitcoin-core-sv2"
cd - > /dev/null || exit 1
echo ""

echo "Running coverage analysis for SV2 Applications..."
echo "================================================="

# stratum-apps Coverage (library crate with all utilities)
echo ""
echo "Running coverage for stratum-apps workspace..."
cd stratum-apps || exit 1
tarpaulin "stratum-apps"
cd - > /dev/null || exit 1

# Pool Coverage
echo ""
echo "Running coverage for pool..."
cd pool-apps/pool || exit 1
tarpaulin "pool"
cd - > /dev/null || exit 1

# JD Server Coverage (separate workspace from pool)
echo ""
echo "Running coverage for jd-server..."
cd pool-apps/jd-server || exit 1
tarpaulin "jd-server"
cd - > /dev/null || exit 1

# Miner Apps Coverage (includes jd-client and translator)
echo ""
echo "Running coverage for miner-apps workspace..."
cd miner-apps || exit 1
tarpaulin "miner-apps"
cd - > /dev/null || exit 1

# Mining Device Coverage (separate workspace from miner-apps)
echo ""
echo "Running coverage for mining-device..."
cd miner-apps/mining-device || exit 1
tarpaulin "mining-device"
cd - > /dev/null || exit 1

echo ""
echo "✅ Coverage analysis completed for all available workspaces."
echo ""
echo "Reports generated:"
echo "  - stratum-apps/target/tarpaulin-reports/stratum-apps-coverage/"
echo "  - pool-apps/pool/target/tarpaulin-reports/pool-coverage/"
echo "  - pool-apps/jd-server/target/tarpaulin-reports/jd-server-coverage/"
echo "  - miner-apps/target/tarpaulin-reports/miner-apps-coverage/"
echo "  - miner-apps/mining-device/target/tarpaulin-reports/mining-device-coverage/"
echo "  - integration-tests/target/tarpaulin-reports/integration-tests-coverage/"