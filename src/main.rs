use candid::{CandidType, Deserialize};
use ic_cdk::api::caller;
use ic_cdk_macros::*;
use std::cell::RefCell;
use std::collections::HashMap;

use candid::Principal;

#[cfg(test)]
mod test_utils {
    use candid::Principal;
    use std::cell::RefCell;

    thread_local! {
        static MOCK_CALLER: RefCell<Principal> = RefCell::new(Principal::anonymous());
        static MOCK_TIME: RefCell<u64> = RefCell::new(0);
    }

    pub fn set_caller(principal: Principal) {
        MOCK_CALLER.with(|caller| *caller.borrow_mut() = principal);
    }

    pub fn get_caller() -> Principal {
        MOCK_CALLER.with(|caller| *caller.borrow())
    }

/*    pub fn set_time(time: u64) {
        MOCK_TIME.with(|t| *t.borrow_mut() = time);
    }
*/
    pub fn get_time() -> u64 {
        MOCK_TIME.with(|t| *t.borrow())
    }
}

#[cfg(test)]
use test_utils::{get_caller, get_time};

#[cfg(not(test))]
fn get_caller() -> Principal {
    ic_cdk::api::caller()
}

#[cfg(not(test))]
fn get_time() -> u64 {
    ic_cdk::api::time()
}

#[derive(CandidType, Deserialize, Clone)]
struct Token {
    name: String,
    symbol: String,
    decimals: u8,
    total_supply: u128,
}

#[derive(CandidType, Deserialize, Clone)]
struct Wallet {
    owner: Principal,
    balances: HashMap<String, u128>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
enum TransferError {
    InsufficientBalance,
    RecipientWalletNotFound,
    SenderWalletNotFound,
    Unauthorized,
    InvalidAmount,
    OverflowError,
}

#[derive(CandidType, Deserialize, Clone)]
struct TransferEvent {
    from: Principal,
    to: Principal,
    amount: u128,
    timestamp: u64,
}

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

fn to_token_units(amount: u128, decimals: u8) -> Result<u128, TransferError> {
    amount.checked_mul(10u128.pow(decimals as u32))
        .ok_or(TransferError::OverflowError)
}

fn is_owner() -> bool {
    let caller = get_caller();
    OWNER.with(|owner| *owner.borrow() == caller)
}

#[init]
fn init() {
    OWNER.with(|owner| *owner.borrow_mut() = caller());
    ic_cdk::println!("Canister initialized with owner: {:?}", caller());
}


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


#[update]
fn transfer(to: Principal, amount: u128) -> Result<bool, TransferError> {
    let caller = get_caller();
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

        *from_balance = from_balance.checked_sub(amount).ok_or(TransferError::OverflowError)?;

        let to_wallet = wallets.entry(to).or_insert(Wallet {
            owner: to,
            balances: HashMap::new(),
        });
        let to_balance = to_wallet.balances.entry("ICPT".to_string()).or_insert(0);
        
        *to_balance = to_balance.checked_add(amount).ok_or(TransferError::OverflowError)?;

        TRANSFER_EVENTS.with(|events| {
            events.borrow_mut().push(TransferEvent {
                from: caller,
                to,
                amount,
                timestamp: get_time(),
            });
        });

        Ok(true)
    })
}

#[update]
fn create_wallet() -> Result<Principal, String> {
    let caller = get_caller();
    println!("Creating wallet for caller: {:?}", caller);
    WALLETS.with(|wallets| {
        let mut wallets = wallets.borrow_mut();
        if wallets.contains_key(&caller) {
            println!("Wallet already exists for caller: {:?}", caller);
            Err("Wallet already exists".to_string())
        } else {
            let new_wallet = Wallet {
                owner: caller,
                balances: HashMap::from([("ICPT".to_string(), 0)]),
            };
            wallets.insert(caller, new_wallet);
            println!("Wallet created successfully for caller: {:?}", caller);
            Ok(caller)
        }
    })
}


#[query]
fn get_transfer_history() -> Vec<TransferEvent> {
    TRANSFER_EVENTS.with(|events| events.borrow().to_vec())
}

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


#[update]
fn burn(amount: u128) -> Result<bool, TransferError> {
    let caller = get_caller();

    ic_cdk::println!("Attempting to burn amount: {}", amount);

    WALLETS.with(|wallets| {
        let mut wallets = wallets.borrow_mut();
        let wallet = wallets.get_mut(&caller).ok_or(TransferError::SenderWalletNotFound)?;
        let balance = wallet.balances.get_mut("ICPT").ok_or(TransferError::InsufficientBalance)?;

        if *balance < amount {
            ic_cdk::println!("Insufficient balance: available {}, trying to burn {}", *balance, amount);
            return Err(TransferError::InsufficientBalance);
        }

        *balance = balance.checked_sub(amount).ok_or(TransferError::OverflowError)?;

        TOKEN.with(|token| {
            let mut token = token.borrow_mut();
            let old_total_supply = token.total_supply;
            let new_total_supply = old_total_supply.checked_sub(amount)
                .ok_or(TransferError::OverflowError)?;

            ic_cdk::println!("Old total supply: {}", old_total_supply);
            ic_cdk::println!("New total supply after burn: {}", new_total_supply);

            token.total_supply = new_total_supply;

            ic_cdk::println!("Updated total supply: {}", token.total_supply);

            if token.total_supply != new_total_supply {
                ic_cdk::println!("Mismatch in total supply update");
                return Err(TransferError::OverflowError); 
            }

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



#[cfg(test)]
fn reset_state() {
    TOKEN.with(|token| {
        *token.borrow_mut() = Token {
            name: "ICP Token".to_string(),
            symbol: "ICPT".to_string(),
            decimals: 8,
            total_supply: 1_000_000_000_000_000_000,
        };
    });
    WALLETS.with(|wallets| wallets.borrow_mut().clear());
    TRANSFER_EVENTS.with(|events| events.borrow_mut().clear());
    OWNER.with(|owner| *owner.borrow_mut() = Principal::anonymous());
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;

    #[test]
    fn test_create_wallet() {
        reset_state();
        let principal = Principal::anonymous();
        test_utils::set_caller(principal);
        let wallets = WALLETS.with(|wallets| wallets.borrow().clone());
        assert!(!wallets.contains_key(&principal));

        assert!(create_wallet().is_ok());
        let wallets = WALLETS.with(|wallets| wallets.borrow().clone());
        assert!(wallets.contains_key(&principal));
    }

    #[test]
    fn test_transfer() {
        reset_state();
        let principal1 = Principal::anonymous();
        let principal2 = Principal::management_canister();

        test_utils::set_caller(principal1);
        assert!(create_wallet().is_ok());
        
        test_utils::set_caller(principal2);
        assert!(create_wallet().is_ok());

        WALLETS.with(|wallets| {
            let mut wallets = wallets.borrow_mut();
            let wallet1 = wallets.get_mut(&principal1).unwrap();
            wallet1.balances.insert("ICPT".to_string(), 100);
        });

        test_utils::set_caller(principal1);
        assert!(transfer(principal2, 50).is_ok());
        assert_eq!(get_balance(principal1), 50);
        assert_eq!(get_balance(principal2), 50);
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        reset_state();
        let principal1 = Principal::anonymous();
        let principal2 = Principal::management_canister();

        test_utils::set_caller(principal1);
        assert!(create_wallet().is_ok());
        
        test_utils::set_caller(principal2);
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
        reset_state();
/*        let principal1 = Principal::anonymous();
        let principal2 = Principal::management_canister();
        let principal3 = Principal::from_text("aaaaa-aa").expect("Failed to create Principal from text");


        let principal1 = Principal::from_text("aaaaa-aa").expect("Failed to create Principal from text");
    let principal2 = Principal::from_text("bbbbb-bb").expect("Failed to create Principal from text");
    let principal3 = Principal::from_text("ccccc-cc").expect("Failed to create Principal from text");
*/

        let principal1 = ic_cdk::id();
        let principal2 = ic_cdk::id();
        let principal3 = ic_cdk::id();

        test_utils::set_caller(principal1);
        assert!(create_wallet().is_ok(), "Failed to create wallet for principal1");
        
        test_utils::set_caller(principal2);
        assert!(create_wallet().is_ok(), "Failed to create wallet for principal2");

        test_utils::set_caller(principal3);
        println!("Attempting to create wallet for principal3: {:?}", principal3);
        assert!(create_wallet().is_ok(), "Failed to create wallet for principal3");

        WALLETS.with(|wallets| {
            let mut wallets = wallets.borrow_mut();
            let wallet1 = wallets.get_mut(&principal1).unwrap();
            wallet1.balances.insert("ICPT".to_string(), 100);
        });

        test_utils::set_caller(principal3);

        assert!(matches!(transfer(principal2, 50), Err(TransferError::InsufficientBalance)), "Unauthorized transfer did not fail as expected");
        assert_eq!(get_balance(principal1), 100);
        assert_eq!(get_balance(principal2), 0);
    }

    #[test]
    fn test_transfer_invalid_amount() {
        reset_state();
        let _principal1 = Principal::anonymous();
        let principal2 = Principal::management_canister();
        
        test_utils::set_caller(_principal1);
        assert!(create_wallet().is_ok());
        
        test_utils::set_caller(principal2);
        assert!(create_wallet().is_ok());

        
        assert!(matches!(transfer(principal2, 0), Err(TransferError::InvalidAmount)));
    }

    #[test]
    fn test_get_balance() {
        reset_state();
        let principal = Principal::anonymous();
        test_utils::set_caller(principal);
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
        reset_state();
        assert_eq!(to_token_units(1, 8).unwrap(), 100_000_000);
        assert_eq!(to_token_units(100, 2).unwrap(), 10_000);
    }


    #[test]
    fn test_mint() {
        reset_state();
        let owner = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
        let recipient = Principal::from_text("aaaaa-aa").unwrap();

        OWNER.with(|o| *o.borrow_mut() = owner);

        test_utils::set_caller(recipient);
        assert!(create_wallet().is_ok());

        assert!(matches!(mint(recipient, 1000), Err(TransferError::Unauthorized)));

        test_utils::set_caller(owner);
        assert!(mint(recipient, 1000).is_ok());

        assert_eq!(get_balance(recipient), 1000);

        TOKEN.with(|token| {
            assert_eq!(token.borrow().total_supply, 1_000_000_000_000_001_000);
        });
    }

    #[test]
    fn test_burn() {
        reset_state();
        let user = Principal::from_text("aaaaa-aa").unwrap();
        test_utils::set_caller(user);
        assert!(create_wallet().is_ok());

        let owner = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
        OWNER.with(|o| *o.borrow_mut() = owner);
        test_utils::set_caller(owner);
        assert!(mint(user, 1000).is_ok());

        test_utils::set_caller(user);
        assert!(burn(500).is_ok());

        assert_eq!(get_balance(user), 500);

        TOKEN.with(|token| {
            assert_eq!(token.borrow().total_supply, 1000000000000000500);
        });
    }

    #[test]
    fn test_burn_insufficient_balance() {
        reset_state();
        let user = Principal::from_text("aaaaa-aa").unwrap();
        test_utils::set_caller(user);
        assert!(create_wallet().is_ok());

        let owner = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
        OWNER.with(|o| *o.borrow_mut() = owner);
        test_utils::set_caller(owner);
        assert!(mint(user, 1000).is_ok());

        test_utils::set_caller(user);
        assert!(matches!(burn(1001), Err(TransferError::InsufficientBalance)));
        
        assert_eq!(get_balance(user), 1000);
        TOKEN.with(|token| {
            assert_eq!(token.borrow().total_supply, 1_000_000_000_000_001_000);
        });
    }

    #[test]
    fn test_mint_overflow() {
        reset_state();
        let owner = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
        let recipient = Principal::from_text("aaaaa-aa").unwrap();

        OWNER.with(|o| *o.borrow_mut() = owner);

        test_utils::set_caller(recipient);
        assert!(create_wallet().is_ok());

        TOKEN.with(|token| {
            token.borrow_mut().total_supply = u128::MAX;
        });

        test_utils::set_caller(owner);
        assert!(matches!(mint(recipient, 1), Err(TransferError::OverflowError)));
    }


    #[test]
    fn test_change_owner() {
        reset_state();
        let initial_owner = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
        let new_owner = Principal::from_text("aaaaa-aa").unwrap();

        OWNER.with(|o| *o.borrow_mut() = initial_owner);

        test_utils::set_caller(new_owner);
        assert!(matches!(change_owner(new_owner), Err(TransferError::Unauthorized)));

        test_utils::set_caller(initial_owner);
        assert!(change_owner(new_owner).is_ok());

        OWNER.with(|o| assert_eq!(*o.borrow(), new_owner));

        test_utils::set_caller(initial_owner);
        assert!(matches!(mint(new_owner, 1000), Err(TransferError::Unauthorized)));

        test_utils::set_caller(new_owner);
        assert!(mint(new_owner, 1000).is_ok());
    }
}

