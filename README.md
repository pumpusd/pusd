# PumpUSD ($PUSD)

> **PumpUSD ($PUSD)** - an experimental, over-collateralized stablecoin designed to maintain a soft peg to the U.S. dollar. Built on Solana for speed, transparency, and programmability.  

---

## Badges
![Build](https://img.shields.io/badge/build-passing-brightgreen)  
![License](https://img.shields.io/badge/license-MIT-blue)  
![Audit](https://img.shields.io/badge/audit-pending-orange)  
![Chain](https://img.shields.io/badge/chain-Solana-purple)

---

## Table of Contents
1. [Overview](#overview)  
2. [Design Goals](#design-goals)  
3. [Architecture](#architecture)  
4. [Key Features](#key-features)  
5. [Getting Started](#getting-started)  

---

## Overview
**PumpUSD (PUSD)** is a research-focused stablecoin protocol that lets users mint **PUSD** by depositing approved collateral into on-chain vaults.  
The system is designed to be transparent, modular, and governed on-chain.  

PUSD is intended as a **decentralized primitive for trading, DeFi applications, and payment rails** within the Solana ecosystem.  

---

## Design Goals
- 💵 **Pegged** — soft peg to USD via over-collateralization  
- 🔍 **Transparent** — all reserves and liabilities on-chain and verifiable  
- 🧩 **Modular** — multi-collateral support with interchangeable oracles  
- 🛡️ **Resilient** — liquidations and circuit breakers prevent runaway risk  
- 🔗 **Composable** — designed to integrate with Solana DeFi protocols  

---

## Architecture
- **PUSD Mint** — SPL Token (Token-2022) controlled by the protocol  
- **Collateral Vaults** — token accounts owned by the program for each supported asset  
- **Oracle Adapters** — integrates Pyth/Switchboard for secure price feeds  
- **Liquidation Engine** — keeper-driven auctions for under-collateralized positions  
- **Governance Layer** — multisig/timelock controls parameter updates  


---

## Key Features
- 🪙 Over-collateralized minting (deposit wSOL/USDC to mint PUSD)  
- 🔒 Health factor checks on every vault  
- ⚡ Fast liquidations with keeper incentives  
- 📊 Global debt ceilings and per-collateral caps  
- ⏸️ Emergency pause switches with timelocks  
- 🌐 Transparent accounting available to anyone via RPC  

---

## Getting Started
### Requirements
- Rust + Cargo  
- Anchor CLI  
- Solana CLI  
- Node.js (for keeper/monitoring scripts)  

### Local Setup
```bash
git clone https://github.com/pumpusd/pusd
cd pumpusd

# Build program
anchor build

# Start local validator
solana-test-validator

# Deploy program
anchor deploy

# Run tests
anchor test
```

