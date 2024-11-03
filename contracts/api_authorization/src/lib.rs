#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;

use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;
use stylus_sdk::{msg, prelude::*};

sol_storage! {
    #[entrypoint]
    pub struct ApiAuthorization {
        mapping(address => uint256) accessings;
    }
}

sol! {
    event Purchase(address addr, uint256 accessings);
}

#[public]
impl ApiAuthorization {
    #[payable]
    pub fn purchase(&mut self) -> U256 {
        let new_accessing = msg::value()
            .checked_div(U256::from(2180330000000000u128))
            .unwrap();
        let final_accessing = self
            .accessings
            .setter(msg::sender())
            .checked_add(new_accessing)
            .unwrap();
        self.accessings.setter(msg::sender()).set(final_accessing);
        self.accessings.get(msg::sender())
    }

    pub fn balance_of(&self, address: Address) -> U256 {
        self.accessings.get(address)
    }

    pub fn mark_usage(&mut self, address: Address) -> U256 {
        let final_accessing = self
            .accessings
            .setter(msg::sender())
            .checked_sub(U256::from(1))
            .unwrap();
        self.accessings.setter(msg::sender()).set(final_accessing);
        self.accessings.get(msg::sender())
    }
}
