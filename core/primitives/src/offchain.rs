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

//! Offchain workers types

use codec::{Encode, Decode};
use rstd::{prelude::{Vec, Box}, convert::TryFrom};
use crate::RuntimeDebug;

pub use crate::crypto::KeyTypeId;

/// A type of supported crypto.
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[repr(C)]
pub enum StorageKind {
	/// Persistent storage is non-revertible and not fork-aware. It means that any value
	/// set by the offchain worker triggered at block `N(hash1)` is persisted even
	/// if that block is reverted as non-canonical and is available for the worker
	/// that is re-run at block `N(hash2)`.
	/// This storage can be used by offchain workers to handle forks
	/// and coordinate offchain workers running on different forks.
	PERSISTENT = 1,
	/// Local storage is revertible and fork-aware. It means that any value
	/// set by the offchain worker triggered at block `N(hash1)` is reverted
	/// if that block is reverted as non-canonical and is NOT available for the worker
	/// that is re-run at block `N(hash2)`.
	LOCAL = 2,
}

impl TryFrom<u32> for StorageKind {
	type Error = ();

	fn try_from(kind: u32) -> Result<Self, Self::Error> {
		match kind {
			e if e == u32::from(StorageKind::PERSISTENT as u8) => Ok(StorageKind::PERSISTENT),
			e if e == u32::from(StorageKind::LOCAL as u8) => Ok(StorageKind::LOCAL),
			_ => Err(()),
		}
	}
}

impl From<StorageKind> for u32 {
	fn from(c: StorageKind) -> Self {
		c as u8 as u32
	}
}

/// Opaque type for offchain http requests.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct HttpRequestId(pub u16);

impl From<HttpRequestId> for u32 {
	fn from(c: HttpRequestId) -> Self {
		c.0 as u32
	}
}

/// An error enum returned by some http methods.
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
#[repr(C)]
pub enum HttpError {
	/// The requested action couldn't been completed within a deadline.
	DeadlineReached = 1,
	/// There was an IO Error while processing the request.
	IoError = 2,
	/// The ID of the request is invalid in this context.
	Invalid = 3,
}

impl TryFrom<u32> for HttpError {
	type Error = ();

	fn try_from(error: u32) -> Result<Self, Self::Error> {
		match error {
			e if e == HttpError::DeadlineReached as u8 as u32 => Ok(HttpError::DeadlineReached),
			e if e == HttpError::IoError as u8 as u32 => Ok(HttpError::IoError),
			e if e == HttpError::Invalid as u8 as u32 => Ok(HttpError::Invalid),
			_ => Err(())
		}
	}
}

impl From<HttpError> for u32 {
	fn from(c: HttpError) -> Self {
		c as u8 as u32
	}
}

/// Status of the HTTP request
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub enum HttpRequestStatus {
	/// Deadline was reached while we waited for this request to finish.
	///
	/// Note the deadline is controlled by the calling part, it not necessarily
	/// means that the request has timed out.
	DeadlineReached,
	/// An error has occured during the request, for example a timeout or the
	/// remote has closed our socket.
	///
	/// The request is now considered destroyed. To retry the request you need
	/// to construct it again.
	IoError,
	/// The passed ID is invalid in this context.
	Invalid,
	/// The request has finished with given status code.
	Finished(u16),
}

impl From<HttpRequestStatus> for u32 {
	fn from(status: HttpRequestStatus) -> Self {
		match status {
			HttpRequestStatus::Invalid => 0,
			HttpRequestStatus::DeadlineReached => 10,
			HttpRequestStatus::IoError => 20,
			HttpRequestStatus::Finished(code) => u32::from(code),
		}
	}
}

impl TryFrom<u32> for HttpRequestStatus {
	type Error = ();

	fn try_from(status: u32) -> Result<Self, Self::Error> {
		match status {
			0 => Ok(HttpRequestStatus::Invalid),
			10 => Ok(HttpRequestStatus::DeadlineReached),
			20 => Ok(HttpRequestStatus::IoError),
			100..=999 => u16::try_from(status).map(HttpRequestStatus::Finished).map_err(|_| ()),
			_ => Err(()),
		}
	}
}

/// A blob to hold information about the local node's network state
/// without committing to its format.
#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
pub struct OpaqueNetworkState {
	/// PeerId of the local node.
	pub peer_id: OpaquePeerId,
	/// List of addresses the node knows it can be reached as.
	pub external_addresses: Vec<OpaqueMultiaddr>,
}

/// Simple blob to hold a `PeerId` without committing to its format.
#[derive(Default, Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
pub struct OpaquePeerId(pub Vec<u8>);

impl OpaquePeerId {
	/// Create new `OpaquePeerId`
	pub fn new(vec: Vec<u8>) -> Self {
		OpaquePeerId(vec)
	}
}

/// Simple blob to hold a `Multiaddr` without committing to its format.
#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
pub struct OpaqueMultiaddr(pub Vec<u8>);

impl OpaqueMultiaddr {
	/// Create new `OpaqueMultiaddr`
	pub fn new(vec: Vec<u8>) -> Self {
		OpaqueMultiaddr(vec)
	}
}

/// Opaque timestamp type
#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Default, RuntimeDebug)]
pub struct Timestamp(u64);

/// Duration type
#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Default, RuntimeDebug)]
pub struct Duration(u64);

impl Duration {
	/// Create new duration representing given number of milliseconds.
	pub fn from_millis(millis: u64) -> Self {
		Duration(millis)
	}

	/// Returns number of milliseconds this Duration represents.
	pub fn millis(&self) -> u64 {
		self.0
	}
}

impl Timestamp {
	/// Creates new `Timestamp` given unix timestamp in miliseconds.
	pub fn from_unix_millis(millis: u64) -> Self {
		Timestamp(millis)
	}

	/// Increase the timestamp by given `Duration`.
	pub fn add(&self, duration: Duration) -> Timestamp {
		Timestamp(self.0.saturating_add(duration.0))
	}

	/// Decrease the timestamp by given `Duration`
	pub fn sub(&self, duration: Duration) -> Timestamp {
		Timestamp(self.0.saturating_sub(duration.0))
	}

	/// Returns a saturated difference (Duration) between two Timestamps.
	pub fn diff(&self, other: &Self) -> Duration {
		Duration(self.0.saturating_sub(other.0))
	}

	/// Return number of milliseconds since UNIX epoch.
	pub fn unix_millis(&self) -> u64 {
		self.0
	}
}

/// Execution context extra capabilities.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum Capability {
	/// Access to transaction pool.
	TransactionPool = 1,
	/// External http calls.
	Http = 2,
	/// Keystore access.
	Keystore = 4,
	/// Randomness source.
	Randomness = 8,
	/// Access to opaque network state.
	NetworkState = 16,
	/// Access to offchain worker DB (read only).
	OffchainWorkerDbRead = 32,
	/// Access to offchain worker DB (writes).
	OffchainWorkerDbWrite = 64,
}

/// A set of capabilities
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Capabilities(u8);

impl Capabilities {
	/// Return an object representing an empty set of capabilities.
	pub fn none() -> Self {
		Self(0)
	}

	/// Return an object representing all capabilities enabled.
	pub fn all() -> Self {
		Self(u8::max_value())
	}

	/// Return capabilities for rich offchain calls.
	///
	/// Those calls should be allowed to sign and submit transactions
	/// and access offchain workers database (but read only!).
	pub fn rich_offchain_call() -> Self {
		[
			Capability::TransactionPool,
			Capability::Keystore,
			Capability::OffchainWorkerDbRead,
		][..].into()
	}

	/// Check if particular capability is enabled.
	pub fn has(&self, capability: Capability) -> bool {
		self.0 & capability as u8 != 0
	}

	/// Check if this capability object represents all capabilities.
	pub fn has_all(&self) -> bool {
		self == &Capabilities::all()
	}
}

impl<'a> From<&'a [Capability]> for Capabilities {
	fn from(list: &'a [Capability]) -> Self {
		Capabilities(list.iter().fold(0_u8, |a, b| a | *b as u8))
	}
}

/// An extended externalities for offchain workers.
pub trait Externalities: Send {
	/// Returns if the local node is a potential validator.
	///
	/// Even if this function returns `true`, it does not mean that any keys are configured
	/// and that the validator is registered in the chain.
	fn is_validator(&self) -> bool;
	/// Submit transaction.
	///
	/// The transaction will end up in the pool and be propagated to others.
	fn submit_transaction(&mut self, extrinsic: Vec<u8>) -> Result<(), ()>;

	/// Returns information about the local node's network state.
	fn network_state(&self) -> Result<OpaqueNetworkState, ()>;

	/// Returns current UNIX timestamp (in millis)
	fn timestamp(&mut self) -> Timestamp;

	/// Pause the execution until `deadline` is reached.
	fn sleep_until(&mut self, deadline: Timestamp);

	/// Returns a random seed.
	///
	/// This is a trully random non deterministic seed generated by host environment.
	/// Obviously fine in the off-chain worker context.
	fn random_seed(&mut self) -> [u8; 32];

	/// Sets a value in the local storage.
	///
	/// Note this storage is not part of the consensus, it's only accessible by
	/// offchain worker tasks running on the same machine. It IS persisted between runs.
	fn local_storage_set(&mut self, kind: StorageKind, key: &[u8], value: &[u8]);

	/// Sets a value in the local storage if it matches current value.
	///
	/// Since multiple offchain workers may be running concurrently, to prevent
	/// data races use CAS to coordinate between them.
	///
	/// Returns `true` if the value has been set, `false` otherwise.
	///
	/// Note this storage is not part of the consensus, it's only accessible by
	/// offchain worker tasks running on the same machine. It IS persisted between runs.
	fn local_storage_compare_and_set(
		&mut self,
		kind: StorageKind,
		key: &[u8],
		old_value: Option<&[u8]>,
		new_value: &[u8],
	) -> bool;

	/// Gets a value from the local storage.
	///
	/// If the value does not exist in the storage `None` will be returned.
	/// Note this storage is not part of the consensus, it's only accessible by
	/// offchain worker tasks running on the same machine. It IS persisted between runs.
	fn local_storage_get(&mut self, kind: StorageKind, key: &[u8]) -> Option<Vec<u8>>;

	/// Initiates a http request given HTTP verb and the URL.
	///
	/// Meta is a future-reserved field containing additional, parity-scale-codec encoded parameters.
	/// Returns the id of newly started request.
	///
	/// Returns an error if:
	/// - No new request identifier could be allocated.
	/// - The method or URI contain invalid characters.
	///
	fn http_request_start(
		&mut self,
		method: &str,
		uri: &str,
		meta: &[u8]
	) -> Result<HttpRequestId, ()>;

	/// Append header to the request.
	///
	/// Calling this function multiple times with the same header name continues appending new
	/// headers. In other words, headers are never replaced.
	///
	/// Returns an error if:
	/// - The request identifier is invalid.
	/// - You have called `http_request_write_body` on that request.
	/// - The name or value contain invalid characters.
	///
	/// An error doesn't poison the request, and you can continue as if the call had never been
	/// made.
	///
	fn http_request_add_header(
		&mut self,
		request_id: HttpRequestId,
		name: &str,
		value: &str
	) -> Result<(), ()>;

	/// Write a chunk of request body.
	///
	/// Calling this function with a non-empty slice may or may not start the
	/// HTTP request. Calling this function with an empty chunks finalizes the
	/// request and always starts it. It is no longer valid to write more data
	/// afterwards.
	/// Passing `None` as deadline blocks forever.
	///
	/// Returns an error if:
	/// - The request identifier is invalid.
	/// - `http_response_wait` has already been called on this request.
	/// - The deadline is reached.
	/// - An I/O error has happened, for example the remote has closed our
	///   request. The request is then considered invalid.
	///
	fn http_request_write_body(
		&mut self,
		request_id: HttpRequestId,
		chunk: &[u8],
		deadline: Option<Timestamp>
	) -> Result<(), HttpError>;

	/// Block and wait for the responses for given requests.
	///
	/// Returns a vector of request statuses (the len is the same as ids).
	/// Note that if deadline is not provided the method will block indefinitely,
	/// otherwise unready responses will produce `DeadlineReached` status.
	///
	/// If a response returns an `IoError`, it is then considered destroyed.
	/// Its id is then invalid.
	///
	/// Passing `None` as deadline blocks forever.
	fn http_response_wait(
		&mut self,
		ids: &[HttpRequestId],
		deadline: Option<Timestamp>
	) -> Vec<HttpRequestStatus>;

	/// Read all response headers.
	///
	/// Returns a vector of pairs `(HeaderKey, HeaderValue)`.
	///
	/// Dispatches the request if it hasn't been done yet. It is no longer
	/// valid to modify the headers or write data to the request.
	///
	/// Returns an empty list if the identifier is unknown/invalid, hasn't
	/// received a response, or has finished.
	fn http_response_headers(
		&mut self,
		request_id: HttpRequestId
	) -> Vec<(Vec<u8>, Vec<u8>)>;

	/// Read a chunk of body response to given buffer.
	///
	/// Dispatches the request if it hasn't been done yet. It is no longer
	/// valid to modify the headers or write data to the request.
	///
	/// Returns the number of bytes written or an error in case a deadline
	/// is reached or server closed the connection.
	/// Passing `None` as a deadline blocks forever.
	///
	/// If `Ok(0)` or `Err(IoError)` is returned, the request is considered
	/// destroyed. Doing another read or getting the response's headers, for
	/// example, is then invalid.
	///
	/// Returns an error if:
	/// - The request identifier is invalid.
	/// - The deadline is reached.
	/// - An I/O error has happened, for example the remote has closed our
	///   request. The request is then considered invalid.
	///
	fn http_response_read_body(
		&mut self,
		request_id: HttpRequestId,
		buffer: &mut [u8],
		deadline: Option<Timestamp>
	) -> Result<usize, HttpError>;

}
impl<T: Externalities + ?Sized> Externalities for Box<T> {
	fn is_validator(&self) -> bool {
		(& **self).is_validator()
	}

	fn submit_transaction(&mut self, ex: Vec<u8>) -> Result<(), ()> {
		(&mut **self).submit_transaction(ex)
	}

	fn network_state(&self) -> Result<OpaqueNetworkState, ()> {
		(& **self).network_state()
	}

	fn timestamp(&mut self) -> Timestamp {
		(&mut **self).timestamp()
	}

	fn sleep_until(&mut self, deadline: Timestamp) {
		(&mut **self).sleep_until(deadline)
	}

	fn random_seed(&mut self) -> [u8; 32] {
		(&mut **self).random_seed()
	}

	fn local_storage_set(&mut self, kind: StorageKind, key: &[u8], value: &[u8]) {
		(&mut **self).local_storage_set(kind, key, value)
	}

	fn local_storage_compare_and_set(
		&mut self,
		kind: StorageKind,
		key: &[u8],
		old_value: Option<&[u8]>,
		new_value: &[u8],
	) -> bool {
		(&mut **self).local_storage_compare_and_set(kind, key, old_value, new_value)
	}

	fn local_storage_get(&mut self, kind: StorageKind, key: &[u8]) -> Option<Vec<u8>> {
		(&mut **self).local_storage_get(kind, key)
	}

	fn http_request_start(&mut self, method: &str, uri: &str, meta: &[u8]) -> Result<HttpRequestId, ()> {
		(&mut **self).http_request_start(method, uri, meta)
	}

	fn http_request_add_header(&mut self, request_id: HttpRequestId, name: &str, value: &str) -> Result<(), ()> {
		(&mut **self).http_request_add_header(request_id, name, value)
	}

	fn http_request_write_body(
		&mut self,
		request_id: HttpRequestId,
		chunk: &[u8],
		deadline: Option<Timestamp>
	) -> Result<(), HttpError> {
		(&mut **self).http_request_write_body(request_id, chunk, deadline)
	}

	fn http_response_wait(&mut self, ids: &[HttpRequestId], deadline: Option<Timestamp>) -> Vec<HttpRequestStatus> {
		(&mut **self).http_response_wait(ids, deadline)
	}

	fn http_response_headers(&mut self, request_id: HttpRequestId) -> Vec<(Vec<u8>, Vec<u8>)> {
		(&mut **self).http_response_headers(request_id)
	}

	fn http_response_read_body(
		&mut self,
		request_id: HttpRequestId,
		buffer: &mut [u8],
		deadline: Option<Timestamp>
	) -> Result<usize, HttpError> {
		(&mut **self).http_response_read_body(request_id, buffer, deadline)
	}
}
/// An `OffchainExternalities` implementation with limited capabilities.
pub struct LimitedExternalities<T> {
	capabilities: Capabilities,
	externalities: T,
}

impl<T> LimitedExternalities<T> {
	/// Create new externalities limited to given `capabilities`.
	pub fn new(capabilities: Capabilities, externalities: T) -> Self {
		Self {
			capabilities,
			externalities,
		}
	}

	/// Check if given capability is allowed.
	///
	/// Panics in case it is not.
	fn check(&self, capability: Capability, name: &'static str) {
		if !self.capabilities.has(capability) {
			panic!("Accessing a forbidden API: {}. No: {:?} capability.", name, capability);
		}
	}
}

impl<T: Externalities> Externalities for LimitedExternalities<T> {
	fn is_validator(&self) -> bool {
		self.check(Capability::Keystore, "is_validator");
		self.externalities.is_validator()
	}

	fn submit_transaction(&mut self, ex: Vec<u8>) -> Result<(), ()> {
		self.check(Capability::TransactionPool, "submit_transaction");
		self.externalities.submit_transaction(ex)
	}

	fn network_state(&self) -> Result<OpaqueNetworkState, ()> {
		self.check(Capability::NetworkState, "network_state");
		self.externalities.network_state()
	}

	fn timestamp(&mut self) -> Timestamp {
		self.check(Capability::Http, "timestamp");
		self.externalities.timestamp()
	}

	fn sleep_until(&mut self, deadline: Timestamp) {
		self.check(Capability::Http, "sleep_until");
		self.externalities.sleep_until(deadline)
	}

	fn random_seed(&mut self) -> [u8; 32] {
		self.check(Capability::Randomness, "random_seed");
		self.externalities.random_seed()
	}

	fn local_storage_set(&mut self, kind: StorageKind, key: &[u8], value: &[u8]) {
		self.check(Capability::OffchainWorkerDbWrite, "local_storage_set");
		self.externalities.local_storage_set(kind, key, value)
	}

	fn local_storage_compare_and_set(
		&mut self,
		kind: StorageKind,
		key: &[u8],
		old_value: Option<&[u8]>,
		new_value: &[u8],
	) -> bool {
		self.check(Capability::OffchainWorkerDbWrite, "local_storage_compare_and_set");
		self.externalities.local_storage_compare_and_set(kind, key, old_value, new_value)
	}

	fn local_storage_get(&mut self, kind: StorageKind, key: &[u8]) -> Option<Vec<u8>> {
		self.check(Capability::OffchainWorkerDbRead, "local_storage_get");
		self.externalities.local_storage_get(kind, key)
	}

	fn http_request_start(&mut self, method: &str, uri: &str, meta: &[u8]) -> Result<HttpRequestId, ()> {
		self.check(Capability::Http, "http_request_start");
		self.externalities.http_request_start(method, uri, meta)
	}

	fn http_request_add_header(&mut self, request_id: HttpRequestId, name: &str, value: &str) -> Result<(), ()> {
		self.check(Capability::Http, "http_request_add_header");
		self.externalities.http_request_add_header(request_id, name, value)
	}

	fn http_request_write_body(
		&mut self,
		request_id: HttpRequestId,
		chunk: &[u8],
		deadline: Option<Timestamp>
	) -> Result<(), HttpError> {
		self.check(Capability::Http, "http_request_write_body");
		self.externalities.http_request_write_body(request_id, chunk, deadline)
	}

	fn http_response_wait(&mut self, ids: &[HttpRequestId], deadline: Option<Timestamp>) -> Vec<HttpRequestStatus> {
		self.check(Capability::Http, "http_response_wait");
		self.externalities.http_response_wait(ids, deadline)
	}

	fn http_response_headers(&mut self, request_id: HttpRequestId) -> Vec<(Vec<u8>, Vec<u8>)> {
		self.check(Capability::Http, "http_response_headers");
		self.externalities.http_response_headers(request_id)
	}

	fn http_response_read_body(
		&mut self,
		request_id: HttpRequestId,
		buffer: &mut [u8],
		deadline: Option<Timestamp>
	) -> Result<usize, HttpError> {
		self.check(Capability::Http, "http_response_read_body");
		self.externalities.http_response_read_body(request_id, buffer, deadline)
	}
}

#[cfg(feature = "std")]
externalities::decl_extension! {
	/// The offchain extension that will be registered at the Substrate externalities.
	pub struct OffchainExt(Box<dyn Externalities>);
}

#[cfg(feature = "std")]
impl OffchainExt {
	/// Create a new instance of `Self`.
	pub fn new<O: Externalities + 'static>(offchain: O) -> Self {
		Self(Box::new(offchain))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn timestamp_ops() {
		let t = Timestamp(5);
		assert_eq!(t.add(Duration::from_millis(10)), Timestamp(15));
		assert_eq!(t.sub(Duration::from_millis(10)), Timestamp(0));
		assert_eq!(t.diff(&Timestamp(3)), Duration(2));
	}

	#[test]
	fn capabilities() {
		let none = Capabilities::none();
		let all = Capabilities::all();
		let some = Capabilities::from(&[Capability::Keystore, Capability::Randomness][..]);

		assert!(!none.has(Capability::Keystore));
		assert!(all.has(Capability::Keystore));
		assert!(some.has(Capability::Keystore));
		assert!(!none.has(Capability::TransactionPool));
		assert!(all.has(Capability::TransactionPool));
		assert!(!some.has(Capability::TransactionPool));
	}
}
