# icp_token_wallet

This Project implements a basic token system on the Internet Computer(IC) platform. It allows for creating wallets, minting token, transferring tokens between wallets, and burning tokens.

## Setup Instructions

1. Ensure you have the DFINITY Canister SDK (dfx) installed. If not, follow the official installation guide: https://sdk.dfinity.org/docs/quickstart/local-quickstart.html

2. Clone this repository to your local machine.

3. Navigate to the project directory in your terminal.

4. Start the local Internet Computer replica:

    **`dfx start --background`**

5. Deploy the canister:
    
    **`dfx deploy`**

## Operational Instructions

### Creating a Wallet

To create a wallet, call the ```create_wallet``` function. This will create a wallet associated with the caller's principal.

**`dfx canister call icp_token create_wallet`**

### Minting Tokens

Only the owner of the canister can mint tokens. To mint tokens:

**`dfx canister call icp_token mint '(principal "<recipient_principal>", <amount>)'`**

Replace ```<recipient_principal>``` with the principal ID of the recipient and ```<amount>``` with the number of tokens to mint.

### Transferring Tokens

To transfer tokens from your wallet to another:

**`dfx canister call icp_token transfer '(principal "<recipient_principal>", <amount>)'`**

### Burning Tokens

To burn tokens from your wallet:

**`dfx canister call icp_token burn <amount>`**

### Checking Balance

To check the balance of a wallet:

**`dfx canister call icp_token get_balance '(principal "<principal_id>")'`**

### Getting Token Info

To get information about the token:

**`dfx canister call icp_token get_token_info`**

### Viewing Transfer History

To view the transfer history:

**`dfx canister call icp_token get_transfer_history`**

### Changing Owner

The current owner can change the ownership of the canister:

**`dfx canister call icp_token change_owner '(principal "<new_owner_principal>")'`**

## Error Handling
The canister implements various error checks:

- Insufficient balance for transfers or burns
- Unauthorized access for minting or changing ownership
- Invalid amounts for transfers
- Overflow errors for large amounts

## Testing

The project includes a comprehensive test suite. To run the tests:

1. Ensure you're in the project directory.
2. Run the following command:

**`cargo test`**

This will execute all the unit tests defined in the ```tests``` module.

## Notes

- All token amounts are represented in the smallest unit of the token (defined by the ```decimals``` field in the token info).
- The initial total supply is set to 1,000,000,000,000,000,000 tokens.
- The canister uses thread-local storage to maintain state, which is suitable for the Internet Computer environment.

## Security Considerations

- Ensure that the private key associated with the owner's principal is kept secure.
- Be cautious when changing ownership, as it transfers full control of the token system.
- Always verify transaction details before approving transfers or burns.


