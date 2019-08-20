use support::{decl_module, decl_storage, decl_event, StorageValue};
use runtime_primitives::traits::As;

pub trait Trait: system::Trait {
	type Event: From<Event> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as QueueManager {
        pub LastBlock: <T as system::Trait>::BlockNumber = Default::default();
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
    pub fn is_wrong_queue(address: &substrate_primitives::sr25519::Public) -> bool {
		let current_block_number: u64 = <LastBlock<T>>::get().as_().saturating_add(1);
		let current_block_number_last_bit = (current_block_number & 1) as u8;
		let last_address_bit = address.as_slice().last().expect("Address should be not empty") & 1;
		return (current_block_number_last_bit ^ last_address_bit) == 1;
    }
}
