# Turbin3 Solana Q3 2026 - Week 2: Escrow Program

A secure and trustless Token Escrow program built on Solana using the Anchor framework, enabling two parties (Maker and Taker) to atomically swap SPL tokens without intermediaries. Tested natively using **LiteSVM**.

---

## 🌟 Architecture Overview

The Escrow contract enables peer-to-peer token exchange:
1. **Maker** deposits `Token A` into a program-owned Associated Token Account (`vault`) and specifies the amount of `Token B` desired in return.
2. An **Escrow State PDA** stores the terms of the trade (`maker`, `mint_a`, `mint_b`, `receive`, `expiration`, `bump`, `seed`).
3. **Taker** completes the exchange atomically by sending `Token B` to the Maker while receiving `Token A` from the vault.
4. If no taker arrives or the maker changes their mind, the Maker can **refund** their tokens or **update** the terms.

### Key PDAs and Accounts

- **`escrow`**: PDA derived from `[b"escrow", maker_pubkey, seed.to_le_bytes()]`. Authority of the token vault.
- **`vault`**: Associated Token Account for `mint_a` owned by the `escrow` PDA.
- **`maker_ata_b` / `taker_ata_a`**: ATAs created `init_if_needed` during the `take` instruction.

---

## 📋 Instructions

| Instruction | Discriminator | Accounts | Description |
| :--- | :---: | :--- | :--- |
| **`make`** | `0` | `maker`, `mint_a`, `mint_b`, `maker_ata_a`, `escrow`, `vault`, programs | Initializes escrow and deposits Token A into vault. |
| **`take`** | `1` | `taker`, `maker`, `mint_a`, `mint_b`, `taker_ata_a`, `taker_ata_b`, `maker_ata_b`, `escrow`, `vault`, programs | Swaps Token B from taker to maker, releases Token A to taker, closes vault and escrow. |
| **`refund`** | `2` | `maker`, `mint_a`, `maker_ata_a`, `escrow`, `vault`, programs | Maker cancels trade, withdraws Token A from vault, closes vault and escrow. |
| **`update`** | `3` | `maker`, `escrow` | Maker updates terms (`receive` amount and `expiration`) without canceling the trade. |

---

## 🧪 Testing with LiteSVM

Tests are written in Rust and execute directly in-memory using **LiteSVM**:

### How to Run

1. **Build the program:**
   ```bash
   anchor build
   ```

2. **Run all tests:**
   ```bash
   cargo test
   ```

### Test Coverage

- ✅ `test_make_and_refund`: Verifies escrow creation, vault deposit, and complete refund liquidation.
- ✅ `test_make_and_take`: Verifies atomic swap between maker and taker, checking that balances update and accounts close cleanly.
- ✅ `test_make_and_update`: Verifies maker can update terms (`receive`, `expiration`) in the escrow PDA.
