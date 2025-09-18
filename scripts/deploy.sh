#!/usr/bin/env bash
# Deploy script for PumpUSD ($PUSD)
# Usage:
#   ./scripts/deploy.sh [localnet|devnet|mainnet] [--upgrade] [--wallet /path/to/id.json]
# Examples:
#   ./scripts/deploy.sh localnet
#   ./scripts/deploy.sh devnet --wallet ~/.config/solana/id.json
#   ./scripts/deploy.sh mainnet --upgrade

set -euo pipefail

PROGRAM_NAME="pusd"                              # must match programs/pusd/Cargo.toml [lib].name
KEYPAIR_PATH="target/deploy/${PROGRAM_NAME}-keypair.json"
ANCHOR_TOML="Anchor.toml"

CLUSTER="${1:-devnet}"                           # default: devnet
ACTION="deploy"                                  # or "upgrade"
WALLET="${HOME}/.config/solana/id.json"

# Parse extra flags
for arg in "$@"; do
  case "$arg" in
    --upgrade) ACTION="upgrade" ;;
    --wallet)  shift; WALLET="${1:-$WALLET}" ;;
  esac
done

echo "==> Cluster: ${CLUSTER}"
echo "==> Action:  ${ACTION}"
echo "==> Wallet:  ${WALLET}"

# 0) Sanity checks
need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing dependency: $1"; exit 1; }; }
need anchor
need solana

if [[ ! -f "$ANCHOR_TOML" ]]; then
  echo "Error: ${ANCHOR_TOML} not found at repo root."
  exit 1
fi

# 1) Configure Solana RPC + wallet
case "$CLUSTER" in
  localnet)
    RPC_URL="http://127.0.0.1:8899"
    ;;
  devnet)
    RPC_URL="https://api.devnet.solana.com"
    ;;
  mainnet)
    RPC_URL="https://api.mainnet-beta.solana.com"
    ;;
  *)
    echo "Unknown cluster: $CLUSTER (use localnet|devnet|mainnet)"
    exit 1
    ;;
esac

echo "==> Setting solana config"
solana config set --url "$RPC_URL" --keypair "$WALLET" >/dev/null

# 2) Confirm mainnet intent
if [[ "$CLUSTER" == "mainnet" ]]; then
  echo "⚠️  You are about to ${ACTION} on MAINNET with wallet: $(solana address)"
  read -r -p "Type 'YES' to continue: " CONFIRM
  [[ "$CONFIRM" == "YES" ]] || { echo "Aborted."; exit 1; }
fi

# 3) Ensure we have funds for tx fees on local/devnet
if [[ "$CLUSTER" != "mainnet" ]]; then
  echo "==> Airdropping 2 SOL to $(solana address) on ${CLUSTER} (if supported)..."
  solana airdrop 2 || echo "(airdrop skipped/not available)"
fi

# 4) Build program
echo "==> Building Anchor workspace"
anchor build

# 5) Determine program ID (from generated keypair)
if [[ -f "$KEYPAIR_PATH" ]]; then
  PROGRAM_ID=$(solana address -k "$KEYPAIR_PATH")
else
  echo "Warning: ${KEYPAIR_PATH} not found. Anchor will create it on first deploy."
  PROGRAM_ID="(unknown until first deploy)"
fi
echo "==> Program: ${PROGRAM_NAME}"
echo "==> Program ID (pre-deploy): ${PROGRAM_ID}"

# 6) Deploy or upgrade
if [[ "$ACTION" == "deploy" ]]; then
  echo "==> Deploying program to ${CLUSTER}"
  case "$CLUSTER" in
    localnet) anchor deploy ;;
    devnet)   anchor deploy --provider.cluster devnet ;;
    mainnet)  anchor deploy --provider.cluster mainnet ;;
  esac
else
  # upgrade
  if [[ "$PROGRAM_ID" == "(unknown until first deploy)" ]]; then
    echo "Error: cannot upgrade before first deploy. Deploy once to create program ID."
    exit 1
  fi
  echo "==> Upgrading program ${PROGRAM_ID} on ${CLUSTER}"
  case "$CLUSTER" in
    localnet) anchor upgrade --program-id "$PROGRAM_ID" target/deploy/${PROGRAM_NAME}.so ;;
    devnet)   anchor upgrade --provider.cluster devnet   --program-id "$PROGRAM_ID" target/deploy/${PROGRAM_NAME}.so ;;
    mainnet)  anchor upgrade --provider.cluster mainnet  --program-id "$PROGRAM_ID" target/deploy/${PROGRAM_NAME}.so ;;
  esac
fi

# 7) Post-deploy: show final IDs and helpful hints
if [[ -f "$KEYPAIR_PATH" ]]; then
  PROGRAM_ID=$(solana address -k "$KEYPAIR_PATH")
  echo "==> Final Program ID: ${PROGRAM_ID}"
fi

echo
echo "✅ Done."
echo "Tips:"
echo "  • Update ${ANCHOR_TOML} [programs.<cluster>].${PROGRAM_NAME} with: ${PROGRAM_ID}"
echo "  • Verify program on-chain: solana program show ${PROGRAM_ID}"
echo "  • Run tests: anchor test"

