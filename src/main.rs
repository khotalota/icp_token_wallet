use candid::{CandidType, Deserialize};
use ic_cdk::api::caller;
use ic_cdk_macros::*;
use std::cell::RefCell;
use std::collections::HashMap;


use candid::Principal;
//use crate::test_utils::set_caller;




#[cfg(test)]
mod test_utils {
    use candid::Principal;
    use std::cell::RefCell;

    thread_local! {
        static MOCK_CALLER: RefCell<Principal> = RefCell::new(Principal::anonymous());
    }

    pub fn set_caller(principal: Principal) {
        MOCK_CALLER.with(|caller| *caller.borrow_mut() = principal);
    }

    pub fn get_caller() -> Principal {
        MOCK_CALLER.with(|caller| *caller.borrow())
    }
}

#[cfg(not(test))]
fn get_caller() -> Principal {
    ic_cdk::api::caller()
}

#[cfg(test)]
use test_utils::get_caller;


// Define the Token struct
#[derive(CandidType, Deserialize, Clone)]
struct Token {
    name: String,
    symbol: String,
    decimals: u8,
    total_supply: u128,
}

// Define the Wallet struct
#[derive(CandidType, Deserialize, Clone)]
struct Wallet {
    owner: Principal,
    balances: HashMap<String, u128>,
}

// Define TransferError for more detailed error handling
#[derive(CandidType, Deserialize, Clone, Debug)]
enum TransferError {
    InsufficientBalance,
    RecipientWalletNotFound,
    SenderWalletNotFound,
    Unauthorized,
    InvalidAmount,
    OverflowError,
}

// Define a TransferEvent for logging
#[derive(CandidType, Deserialize, Clone)]
struct TransferEvent {
    from: Principal,
    to: Principal,
    amount: u128,
    timestamp: u64,
}

// Global state
thread_local! {
    static TOKEN: RefCell<Token> = RefCell::new(Token {
        name: "ICP Token".to_string(),
        symbol: "ICPT".to_string(),
        decimals: 8,
        total_supply: 1_000_000_000_000_000_000,
    });
    static WALLETS: RefCell<HashMap<Principal, Wallet>> = RefCell::new(HashMap::new());
    static TRANSFER_EVENTS: RefCell<Vec<TransferEvent>> = RefCell::new(Vec::new());
    static OWNER: RefCell<Principal> = RefCell::new(Principal::anonymous());
}

// Helper function to convert amount considering decimals
fn to_token_units(amount: u128, decimals: u8) -> Result<u128, TransferError> {
    amount.checked_mul(10u128.pow(decimals as u32))
        .ok_or(TransferError::OverflowError)
}

// Helper function to check if caller is owner
fn is_owner() -> bool {
    let caller = caller();
    OWNER.with(|owner| *owner.borrow() == caller)
}

// Initialize the canister
#[init]
fn init() {
    OWNER.with(|owner| *owner.borrow_mut() = caller());
    ic_cdk::println!("Canister initialized with owner: {:?}", caller());
}

// Query functions

#[query]
fn get_balance(owner: Principal) -> u128 {
    WALLETS.with(|wallets| {
        let wallets = wallets.borrow();
        match wallets.get(&owner) {
            Some(wallet) => wallet.balances.get("ICPT").cloned().unwrap_or(0),
            None => 0,
        }
    })
}

#[query]
fn get_token_info() -> Token {
    TOKEN.with(|token| token.borrow().clone())
}

// Update functions

#[update]
fn transfer(to: Principal, amount: u128) -> Result<bool, TransferError> {
    let caller = caller();
    if amount == 0 {
        return Err(TransferError::InvalidAmount);
    }

    WALLETS.with(|wallets| {
        let mut wallets = wallets.borrow_mut();
        let from_wallet = wallets.get_mut(&caller).ok_or(TransferError::SenderWalletNotFound)?;

        if from_wallet.owner != caller {
            return Err(TransferError::Unauthorized);
        }

        let from_balance = from_wallet.balances.entry("ICPT".to_string()).or_insert(0);
        if *from_balance < amount {
            return Err(TransferError::InsufficientBalance);
        }

        // Safe subtraction
        *from_balance = from_balance.checked_sub(amount).ok_or(TransferError::OverflowError)?;

        let to_wallet = wallets.entry(to).or_insert(Wallet {
            owner: to,
            balances: HashMap::new(),
        });
        let to_balance = to_wallet.balances.entry("ICPT".to_string()).or_insert(0);
        
        // Safe addition
        *to_balance = to_balance.checked_add(amount).ok_or(TransferError::OverflowError)?;

        // Log transfer event
        TRANSFER_EVENTS.with(|events| {
            events.borrow_mut().push(TransferEvent {
                from: caller,
                to,
                amount,
                timestamp: ic_cdk::api::time(),
            });
        });

        Ok(true)
    })
}

#[update]
fn create_wallet() -> Result<Principal, String> {
    let caller = ic_cdk::caller();
    WALLETS.with(|wallets| {
        let mut wallets = wallets.borrow_mut();
        if wallets.contains_key(&caller) {
            Err("Wallet already exists".to_string())
        } else {
            let new_wallet = Wallet {
                owner: caller,
                balances: HashMap::from([("ICPT".to_string(), 0)]),
            };
            wallets.insert(caller, new_wallet);
            Ok(caller)
        }
    })
}

// New query function to get transfer history
#[query]
fn get_transfer_history() -> Vec<TransferEvent> {
    TRANSFER_EVENTS.with(|events| events.borrow().to_vec())
}

// New update function to mint tokens
#[update]
fn mint(to: Principal, amount: u128) -> Result<bool, TransferError> {
    if !is_owner() {
        return Err(TransferError::Unauthorized);
    }

    TOKEN.with(|token| {
        let mut token = token.borrow_mut();
        token.total_supply = token.total_supply.checked_add(amount).ok_or(TransferError::OverflowError)?;
        
        WALLETS.with(|wallets| {
            let mut wallets = wallets.borrow_mut();
            let wallet = wallets.entry(to).or_insert(Wallet {
                owner: to,
                balances: HashMap::new(),
            });
            let balance = wallet.balances.entry("ICPT".to_string()).or_insert(0);
            *balance = balance.checked_add(amount).ok_or(TransferError::OverflowError)?;
            Ok(true)
        })
    })
}

// New update function to burn tokens
#[update]
fn burn(amount: u128) -> Result<bool, TransferError> {
    let caller = caller();
    
    TOKEN.with(|token| {
        let mut token = token.borrow_mut();
        token.total_supply = token.total_supply.checked_sub(amount).ok_or(TransferError::OverflowError)?;
        
        WALLETS.with(|wallets| {
            let mut wallets = wallets.borrow_mut();
            let wallet = wallets.get_mut(&caller).ok_or(TransferError::SenderWalletNotFound)?;
            let balance = wallet.balances.get_mut("ICPT").ok_or(TransferError::InsufficientBalance)?;
            if *balance < amount {
                return Err(TransferError::InsufficientBalance);
            }
            *balance = balance.checked_sub(amount).ok_or(TransferError::OverflowError)?;
            Ok(true)
        })
    })
}

#[update]
fn change_owner(new_owner: Principal) -> Result<(), TransferError> {
    if !is_owner() {
        return Err(TransferError::Unauthorized);
    }
    
    OWNER.with(|owner| {
        *owner.borrow_mut() = new_owner;
    });
    
    ic_cdk::println!("Owner changed to: {:?}", new_owner);
    Ok(())
}

fn main(){}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use ic_cdk::Principal;

    #[test]
    fn test_create_wallet() {
        let principal = Principal::anonymous();
        let wallets = WALLETS.with(|wallets| wallets.borrow().clone());
        assert!(!wallets.contains_key(&principal));

        assert!(create_wallet().is_ok());
        let wallets = WALLETS.with(|wallets| wallets.borrow().clone());
        assert!(wallets.contains_key(&principal));
    }

    #[test]
    fn test_transfer() {
        let principal1 = Principal::anonymous();
        let principal2 = Principal::management_canister();

        assert!(create_wallet().is_ok());
        assert!(create_wallet().is_ok());

        WALLETS.with(|wallets| {
            let mut wallets = wallets.borrow_mut();
            let wallet1 = wallets.get_mut(&principal1).unwrap();
            wallet1.balances.insert("ICPT".to_string(), 100);
        });

        assert!(transfer(principal2, 50).is_ok());
        assert_eq!(get_balance(principal1), 50);
        assert_eq!(get_balance(principal2), 50);
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let principal1 = Principal::anonymous();
        let principal2 = Principal::management_canister();

        assert!(create_wallet().is_ok());
        assert!(create_wallet().is_ok());

        WALLETS.with(|wallets| {
            let mut wallets = wallets.borrow_mut();
            let wallet1 = wallets.get_mut(&principal1).unwrap();
            wallet1.balances.insert("ICPT".to_string(), 100);
        });

        assert!(matches!(transfer(principal2, 150), Err(TransferError::InsufficientBalance)));
        assert_eq!(get_balance(principal1), 100);
        assert_eq!(get_balance(principal2), 0);
    }

    #[test]
    fn test_transfer_unauthorized() {
        let principal1 = Principal::anonymous();
        let principal2 = Principal::management_canister();
        let principal3 = Principal::from_text("aaaaa-aa").unwrap();

        assert!(create_wallet().is_ok());
        assert!(create_wallet().is_ok());
        assert!(create_wallet().is_ok());

        WALLETS.with(|wallets| {
            let mut wallets = wallets.borrow_mut();
            let wallet1 = wallets.get_mut(&principal1).unwrap();
            wallet1.balances.insert("ICPT".to_string(), 100);
        });

        ic_cdk::api::set_caller(principal3);

        assert!(matches!(transfer(principal2, 50), Err(TransferError::Unauthorized)));
        assert_eq!(get_balance(principal1), 100);
        assert_eq!(get_balance(principal2), 0);
    }

    #[test]
    fn test_transfer_invalid_amount() {
        let _principal1 = Principal::anonymous();
        let principal2 = Principal::management_canister();

        assert!(create_wallet().is_ok());
        assert!(create_wallet().is_ok());

        assert!(matches!(transfer(principal2, 0), Err(TransferError::InvalidAmount)));
    }

    #[test]
    fn test_get_balance() {
        let principal = Principal::anonymous();
        assert!(create_wallet().is_ok());

        WALLETS.with(|wallets| {
            let mut wallets = wallets.borrow_mut();
            let wallet = wallets.get_mut(&principal).unwrap();
            wallet.balances.insert("ICPT".to_string(), 100);
        });

        assert_eq!(get_balance(principal), 100);
    }

    #[test]
    fn test_to_token_units() {
        assert_eq!(to_token_units(1, 8).unwrap(), 100_000_000);
        assert_eq!(to_token_units(100, 2).unwrap(), 10_000);
    }


    #[test]
    fn test_mint() {
        let owner = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
        let recipient = Principal::from_text("aaaaa-aa").unwrap();

        // Set the owner
        OWNER.with(|o| *o.borrow_mut() = owner);

        // Create a wallet for the recipient
        ic_cdk::api::set_caller(recipient);
        assert!(create_wallet().is_ok());

        // Attempt to mint as non-owner (should fail)
        assert!(matches!(mint(recipient, 1000), Err(TransferError::Unauthorized)));

        // Mint as owner
        ic_cdk::api::set_caller(owner);
        assert!(mint(recipient, 1000).is_ok());

        // Check the balance
        assert_eq!(get_balance(recipient), 1000);

        // Check total supply
        TOKEN.with(|token| {
            assert_eq!(token.borrow().total_supply, 1_000_000_000_000_001_000);
        });
    }

    #[test]
    fn test_burn() {
        let user = Principal::from_text("aaaaa-aa").unwrap();

        // Create a wallet for the user
        ic_cdk::api::set_caller(user);
        assert!(create_wallet().is_ok());

        // Mint some tokens for the user (as owner)
        let owner = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
        OWNER.with(|o| *o.borrow_mut() = owner);
        ic_cdk::api::set_caller(owner);
        assert!(mint(user, 1000).is_ok());

        // Attempt to burn more than balance (should fail)
        ic_cdk::api::set_caller(user);
        assert!(matches!(burn(1001), Err(TransferError::InsufficientBalance)));

        // Burn some tokens
        assert!(burn(500).is_ok());

        // Check the balance
        assert_eq!(get_balance(user), 500);

        // Check total supply
        TOKEN.with(|token| {
            assert_eq!(token.borrow().total_supply, 1_000_000_000_000_000_500);
        });
    }

    #[test]
    fn test_mint_overflow() {
        let owner = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
        let recipient = Principal::from_text("aaaaa-aa").unwrap();

        // Set the owner
        OWNER.with(|o| *o.borrow_mut() = owner);

        // Create a wallet for the recipient
        ic_cdk::api::set_caller(recipient);
        assert!(create_wallet().is_ok());

        // Set total supply to maximum value
        TOKEN.with(|token| {
            token.borrow_mut().total_supply = u128::MAX;
        });

        // Attempt to mint (should fail due to overflow)
        ic_cdk::api::set_caller(owner);
        assert!(matches!(mint(recipient, 1), Err(TransferError::OverflowError)));
    }


    #[test]
    fn test_change_owner() {
        let initial_owner = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
        let new_owner = Principal::from_text("aaaaa-aa").unwrap();

        // Set the initial owner
        OWNER.with(|o| *o.borrow_mut() = initial_owner);

        // Attempt to change owner as non-owner (should fail)
        ic_cdk::api::set_caller(new_owner);
        assert!(matches!(change_owner(new_owner), Err(TransferError::Unauthorized)));

        // Change owner as current owner
        ic_cdk::api::set_caller(initial_owner);
        assert!(change_owner(new_owner).is_ok());

        // Verify the owner has been changed
        OWNER.with(|o| assert_eq!(*o.borrow(), new_owner));

        // Attempt to perform owner-only action with old owner (should fail)
        ic_cdk::api::set_caller(initial_owner);
        assert!(matches!(mint(new_owner, 1000), Err(TransferError::Unauthorized)));

        // Perform owner-only action with new owner (should succeed)
        ic_cdk::api::set_caller(new_owner);
        assert!(mint(new_owner, 1000).is_ok());
    }
}

