// Copyright 2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Abstract storage to use on HashedStorage trait. Please refer to the
//! [top level docs](../../index.html) for more detailed documentation about storage traits and functions.

use crate::codec::{self, Encode};
use crate::rstd::prelude::{Vec, Box};
#[cfg(feature = "std")]
use crate::storage::unhashed::generator::UnhashedStorage;
use runtime_io::{twox_64, twox_128, blake2_128, twox_256, blake2_256};

pub trait StorageHasher: 'static {
	type Output: AsRef<[u8]>;
	fn hash(x: &[u8]) -> Self::Output;
}

/// Hash storage keys with `concat(twox64(key), key)`
pub struct Twox64Concat;
impl StorageHasher for Twox64Concat {
	type Output = Vec<u8>;
	fn hash(x: &[u8]) -> Vec<u8> {
		twox_64(x)
			.into_iter()
			.chain(x.into_iter())
			.cloned()
			.collect::<Vec<_>>()
	}
}

#[test]
fn test_twox_64_concat() {
	let r = Twox64Concat::hash(b"foo");
	assert_eq!(r.split_at(8), (&twox_128(b"foo")[..8], &b"foo"[..]))
}

/// Hash storage keys with blake2 128
pub struct Blake2_128;
impl StorageHasher for Blake2_128 {
	type Output = [u8; 16];
	fn hash(x: &[u8]) -> [u8; 16] {
		blake2_128(x)
	}
}

/// Hash storage keys with blake2 256
pub struct Blake2_256;
impl StorageHasher for Blake2_256 {
	type Output = [u8; 32];
	fn hash(x: &[u8]) -> [u8; 32] {
		blake2_256(x)
	}
}

/// Hash storage keys with twox 128
pub struct Twox128;
impl StorageHasher for Twox128 {
	type Output = [u8; 16];
	fn hash(x: &[u8]) -> [u8; 16] {
		twox_128(x)
	}
}

/// Hash storage keys with twox 256
pub struct Twox256;
impl StorageHasher for Twox256 {
	type Output = [u8; 32];
	fn hash(x: &[u8]) -> [u8; 32] {
		twox_256(x)
	}
}

/// Abstraction around storage.
pub trait HashedStorage<H: StorageHasher> {
	/// true if the key exists in storage.
	fn exists(&self, key: &[u8]) -> bool;

	/// Load the bytes of a key from storage. Can panic if the type is incorrect.
	fn get<T: codec::Decode>(&self, key: &[u8]) -> Option<T>;

	/// Load the bytes of a key from storage. Can panic if the type is incorrect. Will panic if
	/// it's not there.
	fn require<T: codec::Decode>(&self, key: &[u8]) -> T {
		self.get(key).expect("Required values must be in storage")
	}

	/// Load the bytes of a key from storage. Can panic if the type is incorrect. The type's
	/// default is returned if it's not there.
	fn get_or_default<T: codec::Decode + Default>(&self, key: &[u8]) -> T {
		self.get(key).unwrap_or_default()
	}

	/// Put a value in under a key.
	fn put<T: codec::Encode>(&mut self, key: &[u8], val: &T);

	/// Remove the bytes of a key from storage.
	fn kill(&mut self, key: &[u8]);

	/// Take a value from storage, deleting it after reading.
	fn take<T: codec::Decode>(&mut self, key: &[u8]) -> Option<T> {
		let value = self.get(key);
		self.kill(key);
		value
	}

	/// Take a value from storage, deleting it after reading.
	fn take_or_panic<T: codec::Decode>(&mut self, key: &[u8]) -> T {
		self.take(key).expect("Required values must be in storage")
	}

	/// Take a value from storage, deleting it after reading.
	fn take_or_default<T: codec::Decode + Default>(&mut self, key: &[u8]) -> T {
		self.take(key).unwrap_or_default()
	}

	/// Get a Vec of bytes from storage.
	fn get_raw(&self, key: &[u8]) -> Option<Vec<u8>>;

	/// Put a raw byte slice into storage.
	fn put_raw(&mut self, key: &[u8], value: &[u8]);
}

// We use a construct like this during when genesis storage is being built.
#[cfg(feature = "std")]
impl<H: StorageHasher> HashedStorage<H> for sr_primitives::StorageOverlay {
	fn exists(&self, key: &[u8]) -> bool {
		UnhashedStorage::exists(self, &H::hash(key).as_ref())
	}

	fn get<T: codec::Decode>(&self, key: &[u8]) -> Option<T> {
		UnhashedStorage::get(self, &H::hash(key).as_ref())
	}

	fn put<T: codec::Encode>(&mut self, key: &[u8], val: &T) {
		UnhashedStorage::put(self, &H::hash(key).as_ref(), val)
	}

	fn kill(&mut self, key: &[u8]) {
		UnhashedStorage::kill(self, &H::hash(key).as_ref())
	}

	fn get_raw(&self, key: &[u8]) -> Option<Vec<u8>> {
		UnhashedStorage::get_raw(self, &H::hash(key).as_ref())
	}

	fn put_raw(&mut self, key: &[u8], value: &[u8]) {
		UnhashedStorage::put_raw(self, &H::hash(key).as_ref(), value)
	}
}

/// A strongly-typed value kept in storage.
pub trait StorageValue<T: codec::Codec> {
	/// The type that get/take returns.
	type Query;

	/// Get the storage key.
	fn key() -> &'static [u8];

	/// true if the value is defined in storage.
	fn exists<S: HashedStorage<Twox128>>(storage: &S) -> bool {
		storage.exists(Self::key())
	}

	/// Load the value from the provided storage instance.
	fn get<S: HashedStorage<Twox128>>(storage: &S) -> Self::Query;

	/// Take a value from storage, removing it afterwards.
	fn take<S: HashedStorage<Twox128>>(storage: &mut S) -> Self::Query;

	/// Store a value under this key into the provided storage instance.
	fn put<S: HashedStorage<Twox128>>(val: &T, storage: &mut S) {
		storage.put(Self::key(), val)
	}

	/// Store a value under this key into the provided storage instance; this can take any reference
	/// type that derefs to `T` (and has `Encode` implemented).
	/// Store a value under this key into the provided storage instance.
	fn put_ref<Arg: ?Sized + Encode, S: HashedStorage<Twox128>>(val: &Arg, storage: &mut S) where T: AsRef<Arg> {
		val.using_encoded(|b| storage.put_raw(Self::key(), b))
	}

	/// Mutate this value
	fn mutate<R, F: FnOnce(&mut Self::Query) -> R, S: HashedStorage<Twox128>>(f: F, storage: &mut S) -> R;

	/// Clear the storage value.
	fn kill<S: HashedStorage<Twox128>>(storage: &mut S) {
		storage.kill(Self::key())
	}

	/// Append the given items to the value in the storage.
	///
	/// `T` is required to implement `codec::EncodeAppend`.
	///
	/// This can only be called for storage items that do not have a default values.
	fn append<S: HashedStorage<Twox128>, I: codec::Encode>(
		items: &[I],
		storage: &mut S,
	) -> Result<(), &'static str>
		where T: codec::EncodeAppend<Item=I>, Self: crate::traits::NoDefault,
	{
		let new_val = <T as codec::EncodeAppend>::append(
			storage.get_raw(Self::key()).unwrap_or_default(),
			items,
		).map_err(|_| "Could not append given item")?;
		storage.put_raw(Self::key(), &new_val);
		Ok(())
	}

	/// Safely append the given items to the value in the storage.
	///
	/// `T` is required to implement `codec::EncodeAppend`.
	///
	/// This can only be called for storage items that do not have a default values.
	fn safe_append<'a, S: HashedStorage<Twox128>, I>(
		items: &'a[I],
		storage: &mut S,
	) where
		Self: crate::traits::NoDefault,
		T: codec::EncodeAppend<Item=I> + From<&'a[I]>,
		I: 'a + codec::Encode + Clone,
	{
		Self::append(items, storage).unwrap_or_else(|_| Self::put(&items.clone().into(), storage));
	}

	/// Read the length of the value in a fast way, without decoding the entire value.
	///
	/// `T` is required to implement `Codec::DecodeLength`.
	///
	/// This can only be called for storage items that do not have a default values.
	fn decode_len<S: HashedStorage<Twox128>>(storage: &mut S) -> Result<usize, &'static str>
		where T: codec::DecodeLength, Self: crate::traits::NoDefault,
	{
		// attempt to get the length directly.
		if let Some(k) = storage.get_raw(Self::key()) {
			let l = <T as codec::DecodeLength>::len(&k).map_err(|_| "could not decode length")?;
			Ok(l)
		} else {
			Err("could not find item to decode length")
		}
	}
}

/// A strongly-typed map in storage.
pub trait StorageMap<K: codec::Codec, V: codec::Codec> {
	/// The type that get/take returns.
	type Query;

	type Hasher: StorageHasher;

	/// Get the prefix key in storage.
	fn prefix() -> &'static [u8];

	/// Get the storage key used to fetch a value corresponding to a specific key.
	fn key_for(x: &K) -> Vec<u8>;

	/// true if the value is defined in storage.
	fn exists<S: HashedStorage<Self::Hasher>>(key: &K, storage: &S) -> bool {
		storage.exists(&Self::key_for(key)[..])
	}

	/// Load the value associated with the given key from the map.
	fn get<S: HashedStorage<Self::Hasher>>(key: &K, storage: &S) -> Self::Query;

	/// Take the value under a key.
	fn take<S: HashedStorage<Self::Hasher>>(key: &K, storage: &mut S) -> Self::Query;

	/// Swap the values of two keys.
	fn swap<S: HashedStorage<Self::Hasher>>(key1: &K, key2: &K, storage: &mut S) {
		let k1 = Self::key_for(key1);
		let k2 = Self::key_for(key2);
		let v1 = storage.get_raw(&k1[..]);
		if let Some(val) = storage.get_raw(&k2[..]) {
			storage.put_raw(&k1[..], &val[..]);
		} else {
			storage.kill(&k1[..])
		}
		if let Some(val) = v1 {
			storage.put_raw(&k2[..], &val[..]);
		} else {
			storage.kill(&k2[..])
		}
	}

	/// Store a value to be associated with the given key from the map.
	fn insert<S: HashedStorage<Self::Hasher>>(key: &K, val: &V, storage: &mut S) {
		storage.put(&Self::key_for(key)[..], val);
	}

	/// Store a value under this key into the provided storage instance; this can take any reference
	/// type that derefs to `T` (and has `Encode` implemented).
	/// Store a value under this key into the provided storage instance.
	fn insert_ref<Arg: ?Sized + Encode, S: HashedStorage<Self::Hasher>>(
		key: &K,
		val: &Arg,
		storage: &mut S
	) where V: AsRef<Arg> {
		val.using_encoded(|b| storage.put_raw(&Self::key_for(key)[..], b))
	}

	/// Remove the value under a key.
	fn remove<S: HashedStorage<Self::Hasher>>(key: &K, storage: &mut S) {
		storage.kill(&Self::key_for(key)[..]);
	}

	/// Mutate the value under a key.
	fn mutate<R, F: FnOnce(&mut Self::Query) -> R, S: HashedStorage<Self::Hasher>>(key: &K, f: F, storage: &mut S) -> R;
}

/// A `StorageMap` with enumerable entries.
pub trait EnumerableStorageMap<K: codec::Codec, V: codec::Codec>: StorageMap<K, V> {
	/// Return current head element.
	fn head<S: HashedStorage<Self::Hasher>>(storage: &S) -> Option<K>;

	/// Enumerate all elements in the map.
	fn enumerate<'a, S: HashedStorage<Self::Hasher>>(
		storage: &'a S
	) -> Box<dyn Iterator<Item = (K, V)> + 'a> where K: 'a, V: 'a;
}

/// A `StorageMap` with appendable entries.
pub trait AppendableStorageMap<K: codec::Codec, V: codec::Codec>: StorageMap<K, V> {
	/// Append the given items to the value in the storage.
	///
	/// `V` is required to implement `codec::EncodeAppend`.
	fn append<S: HashedStorage<Self::Hasher>, I: codec::Encode>(
		key : &K,
		items: &[I],
		storage: &mut S
	) -> Result<(), &'static str>
		where
			V: codec::EncodeAppend<Item=I>,
			Self: crate::traits::NoDefault,
	{
		let k = Self::key_for(key);
		let new_val = <V as codec::EncodeAppend>::append(
			storage.get_raw(&k[..]).unwrap_or_default(),
			items,
		).map_err(|_| "Could not append given item")?;
		storage.put_raw(&k[..], &new_val);
		Ok(())
	}

	/// Safely append the given items to the value in the storage.
	///
	/// `T` is required to implement `codec::EncodeAppend`.
	///
	/// This can only be called for storage items that do not have a default values.
	fn safe_append<'a, S, I>(
		key : &K,
		items: &'a[I],
		storage: &mut S,
	) where
		Self: crate::traits::NoDefault,
		S: HashedStorage<Self::Hasher>,
		I: codec::Encode + Clone,
		V: codec::EncodeAppend<Item=I> + From<&'a[I]>,
	{
		Self::append(key, items, storage)
			.unwrap_or_else(|_| Self::insert(key, &items.clone().into(), storage));
	}
}

/// A storage map with a decodable length.
pub trait DecodeLengthStorageMap<K: codec::Codec, V: codec::Codec>: StorageMap<K, V> {
	/// Read the length of the value in a fast way, without decoding the entire value.
	///
	/// `T` is required to implement `Codec::DecodeLength`.
	///
	/// This can only be called for storage items that do not have a default values.
	fn decode_len<S: HashedStorage<Self::Hasher>>(key: &K, storage: &mut S) -> Result<usize, &'static str>
		where V: codec::DecodeLength, Self: crate::traits::NoDefault
	{
		let k = Self::key_for(key);
		if let Some(v) = storage.get_raw(&k[..]) {
			let l = <V as codec::DecodeLength>::len(&v).map_err(|_| "could not decode length")?;
			Ok(l)
		} else {
			Err("could not find item to decode length")
		}
	}
}
