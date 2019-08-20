use support::{decl_module, decl_storage, decl_event, StorageValue, dispatch::Result};
use node_primitives::BlockNumber;

pub trait Trait: system::Trait {
	type Event: From<Event> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as QueueManager {
        pub LastBlock: <T as system::Trait>::BlockNumber;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn on_finalize(block_number: <T as system::Trait>::BlockNumber) {
			<LastBlock<T>>::put(block_number);
        }
    }
}

decl_event!(
    pub enum Event {
		Noop,
	}
);

impl<T: Trait> Module<T> {
    pub fn verify_queue(address: &substrate_primitives::sr25519::Public) -> bool {
		// let is_odd_block = !<IsOddQueue<T>>::get().expect("Unexpected missing storage value for IsOddQueue");

		// if let Some(sender) = transaction.sender() {
		// 	let last_bit = sender.as_slice.last() & 1
		// 	let oddity = is_odd_block as u32;
		// 	return (last_bit ^ oddity) == 0;
		// }

		return false;
    }
}
