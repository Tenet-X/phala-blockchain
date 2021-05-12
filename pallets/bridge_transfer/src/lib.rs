// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::{Currency, EnsureOrigin, ExistenceRequirement::AllowDeath, Get};
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
};
use frame_system::{self as system, ensure_signed};
use pallet_bridge as bridge;
use sp_arithmetic::traits::SaturatedConversion;
use sp_core::U256;
use sp_std::prelude::*;
use codec::{Encode, Decode};

mod mock;
mod tests;

type ResourceId = bridge::ResourceId;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub trait Config: system::Config + bridge::Config {
	type Event: From<Event> + Into<<Self as frame_system::Config>::Event>;

	/// Specifies the origin check provided by the bridge for calls that can only be called by the bridge pallet
	type BridgeOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;

	/// The currency mechanism.
	type Currency: Currency<Self::AccountId>;
}

decl_storage! {
	trait Store for Module<T: Config> as BridgeTransfer {
		BridgeTokenId get(fn bridge_tokenid): ResourceId;
		BridgeLotteryId get(fn bridge_lotteryid): ResourceId;
	}

	add_extra_genesis {
		config(bridge_tokenid): ResourceId;
		config(bridge_lotteryid): ResourceId;
		build(|config: &GenesisConfig| {
			BridgeTokenId::put(config.bridge_tokenid);
			BridgeLotteryId::put(config.bridge_lotteryid);
		});
	}
}

decl_event! {
	pub enum Event {
		/// Received BTC lottery encoded command
		LotteryCommand(Vec<u8>),
		/// A signed BTC transaction was send. [dest_chain, resource_id, payload]
		BTCSignedTxSend(bridge::ChainId, ResourceId, Vec<u8>),
	}
}

decl_error! {
	pub enum Error for Module<T: Config>{
		InvalidTransfer,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		//
		// Initiation calls. These start a bridge transfer.
		//

		/// Transfers an arbitrary signed bitcoin tx to a (whitelisted) destination chain.
		#[weight = 195_000_000]
		pub fn transfer_lottery(origin, round_id: u32, token_id: u32, payload: Vec<u8>, dest_id: bridge::ChainId) -> DispatchResult {
			ensure_signed(origin)?;

			let resource_id = Self::bridge_lotteryid();

			let payload_len: u32 = payload.len().saturated_into();
			let mut encoded_payload_len: Vec<u8> = payload_len.encode();
			encoded_payload_len.reverse();

			let mut encoded_round_id: Vec<u8> = round_id.encode();
			encoded_round_id.reverse();

			let mut encoded_token_id: Vec<u8> = token_id.encode();
			encoded_token_id.reverse();

			let metadata: Vec<u8> = [encoded_round_id, encoded_token_id, encoded_payload_len, payload.clone()].concat();

			Self::deposit_event(Event::BTCSignedTxSend(dest_id, resource_id, payload));

			<bridge::Module<T>>::transfer_generic(dest_id, resource_id, metadata)
		}

		/// Transfers some amount of the native token to some recipient on a (whitelisted) destination chain.
		#[weight = 195_000_000]
		pub fn transfer_native(origin, amount: BalanceOf<T>, recipient: Vec<u8>, dest_id: bridge::ChainId) -> DispatchResult {
			let source = ensure_signed(origin)?;
			ensure!(<bridge::Module<T>>::chain_whitelisted(dest_id), Error::<T>::InvalidTransfer);
			let bridge_id = <bridge::Module<T>>::account_id();
			T::Currency::transfer(&source, &bridge_id, amount.into(), AllowDeath)?;

			let resource_id = Self::bridge_tokenid();

			<bridge::Module<T>>::transfer_fungible(dest_id, resource_id, recipient, U256::from(amount.saturated_into::<u128>()))
		}

		//
		// Executable calls. These can be triggered by a bridge transfer initiated on another chain
		//

		/// Executes a simple currency transfer using the bridge account as the source
		#[weight = 195_000_000]
		pub fn transfer(origin, to: T::AccountId, amount: BalanceOf<T>, rid: ResourceId) -> DispatchResult {
			let source = T::BridgeOrigin::ensure_origin(origin)?;
			<T as Config>::Currency::transfer(&source, &to, amount.into(), AllowDeath)?;
			Ok(())
		}

		/// This can be called by the bridge to demonstrate an arbitrary call from a proposal.
		#[weight = 195_000_000]
		pub fn lottery_handler(origin, metadata: Vec<u8>, rid: ResourceId) -> DispatchResult {
			T::BridgeOrigin::ensure_origin(origin)?;

			Self::deposit_event(Event::LotteryCommand(metadata));

			// TODO: Send to BTC Lottery pallet to handle this

			Ok(())
		}
	}
}
