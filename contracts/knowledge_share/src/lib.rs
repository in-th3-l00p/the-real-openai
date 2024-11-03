// Allow `cargo stylus export-abi` to generate a main function.
#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;

use alloy_primitives::U256;
use alloy_sol_types::sol;
use stylus_sdk::{evm, msg, prelude::*};

const PUSH_LIMIT: usize = 3;

sol! {
    event LimitReached(string[] shares);
}

sol_storage! {
    #[entrypoint]
    pub struct KnowledgeShare {
        address owner;
        bool rewarded;

        string[] shares;
        mapping(uint256 => address) share_address;
        mapping(address => uint256) successful_shares;
    }
}



#[public]
impl KnowledgeShare {
    pub fn set_owner(&mut self) -> bool {
        if !self.owner.is_zero() {
            return false;
        }
        self.owner.set(msg::sender());
        self.rewarded.set(true);
        true
    }

    pub fn is_reward_in_progress(&self) -> bool {
        !self.rewarded.get()
    }

    pub fn share(&mut self, knowledge: String) {
        if !self.rewarded.get() {
            return ();
        }
        let mut new_share = self.shares.grow();
        new_share.set_str(knowledge);

        if self.shares.len() >= PUSH_LIMIT {
            self.rewarded.set(false);
            let mut local_shares: Vec<String> = Vec::new();
            for share in 0..self.shares.len() {
                local_shares.push(self.shares.get(share).unwrap().get_string());
            }

            evm::log(LimitReached {
                shares: local_shares
            });
        }
    }
    
    pub fn get_submitted_knowledge(&self) -> Vec<String> {
        let mut local_shares: Vec<String> = Vec::new();
        for share in 0..self.shares.len() {
            local_shares.push(self.shares.get(share).unwrap().get_string());
        }
        local_shares
    }

    pub fn reward(&mut self, valids: Vec<bool>) {
        if !msg::sender().eq(&self.owner.get()) {
            return ();
        }

        for index in 0..valids.len() {
            if valids[index] {
                let new_successful_shares = self
                    .successful_shares
                    .setter(self.share_address.get(U256::from(index)))
                    .checked_add(U256::from(1))
                    .unwrap();
                self
                    .successful_shares
                    .setter(self.share_address.get(U256::from(index)))
                    .set(new_successful_shares);
            }
        }

        ()
    }
}
