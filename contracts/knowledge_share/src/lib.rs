// Allow `cargo stylus export-abi` to generate a main function.
#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;

use alloy_primitives::{U256, Address, Uint};
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
        uint256[3] votes;
        mapping(address => uint256) vote_address;
        mapping(uint256 => address) share_address;
        mapping(address => uint256) successful_shares;

        string[] knowledge;
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
                self.votes.setter(share).unwrap().set(U256::from(0));
            }

            evm::log(LimitReached {
                shares: local_shares
            });
        }
    }

    pub fn get_submitted_knowledge(&self) -> (Vec<Address>, Vec<String>) {
        let mut local_shares: Vec<String> = Vec::new();
        let mut addresses: Vec<Address> = Vec::new();
        for share in 0..self.shares.len() {
            addresses.push(self.share_address.get(U256::from(share)));
            local_shares.push(self.shares.get(share).unwrap().get_string());
        }
        (addresses, local_shares)
    }

    pub fn get_vote(&self) -> Uint<256, 4> {
        self.vote_address.get(msg::sender())
    }

    pub fn vote(&mut self, index: U256) {
        if
            self.rewarded.get() ||
            index.lt(&U256::from(0)) ||
            index.gt(&U256::from(PUSH_LIMIT)) ||
            !self.vote_address.get(msg::sender()).eq(&U256::from(0))
        {
            return ();
        }

        let new_value = self.votes
            .setter(index)
            .unwrap()
            .checked_add(U256::from(1))
            .unwrap();
        self.votes.setter(index).unwrap().set(new_value);
        self.vote_address
            .setter(msg::sender())
            .set(index.checked_add(U256::from(1)).unwrap());
        if self.votes.get(index).unwrap() > U256::from(PUSH_LIMIT) {
            let new_successful_shares = self
                .successful_shares
                .setter(self.share_address.get(U256::from(index)))
                .checked_add(U256::from(1))
                .unwrap();
            self
                .successful_shares
                .setter(self.share_address.get(U256::from(index)))
                .set(new_successful_shares);
            self.rewarded.set(false);
        }
    }
}
