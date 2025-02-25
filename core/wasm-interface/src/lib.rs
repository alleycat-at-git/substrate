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

//! Types and traits for interfacing between the host and the wasm runtime.

use std::{borrow::Cow, marker::PhantomData, mem, iter::Iterator, result};

mod wasmi_impl;

/// Result type used by traits in this crate.
pub type Result<T> = result::Result<T, String>;

/// Value types supported by Substrate on the boundary between host/Wasm.
#[derive(Copy, Clone, PartialEq, Debug, Eq)]
pub enum ValueType {
	/// An `i32` value type.
	I32,
	/// An `i64` value type.
	I64,
	/// An `f32` value type.
	F32,
	/// An `f64` value type.
	F64,
}

/// Values supported by Substrate on the boundary between host/Wasm.
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Value {
	/// An `i32` value.
	I32(i32),
	/// An `i64` value.
	I64(i64),
	/// An nan-preserving `f32` value.
	F32(u32),
	/// An nan-preserving `f64` value.
	F64(u64),
}

impl Value {
	/// Returns the type of this value.
	pub fn value_type(&self) -> ValueType {
		match self {
			Value::I32(_) => ValueType::I32,
			Value::I64(_) => ValueType::I64,
			Value::F32(_) => ValueType::F32,
			Value::F64(_) => ValueType::F64,
		}
	}
}

/// Provides `Sealed` trait to prevent implementing trait `PointerType` outside of this crate.
mod private {
	pub trait Sealed {}

	impl Sealed for u8 {}
	impl Sealed for u16 {}
	impl Sealed for u32 {}
	impl Sealed for u64 {}
}

/// Something that can be wrapped in a wasm `Pointer`.
///
/// This trait is sealed.
pub trait PointerType: Sized {
	/// The size of the type in wasm.
	const SIZE: u32 = mem::size_of::<Self>() as u32;
}

impl PointerType for u8 {}
impl PointerType for u16 {}
impl PointerType for u32 {}
impl PointerType for u64 {}

/// Type to represent a pointer in wasm at the host.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Pointer<T: PointerType> {
	ptr: u32,
	_marker: PhantomData<T>,
}

impl<T: PointerType> Pointer<T> {
	/// Create a new instance of `Self`.
	pub fn new(ptr: u32) -> Self {
		Self {
			ptr,
			_marker: Default::default(),
		}
	}

	/// Calculate the offset from this pointer.
	///
	/// `offset` is in units of `T`. So, `3` means `3 * mem::size_of::<T>()` as offset to the pointer.
	///
	/// Returns an `Option` to respect that the pointer could probably overflow.
	pub fn offset(self, offset: u32) -> Option<Self> {
		offset.checked_mul(T::SIZE).and_then(|o| self.ptr.checked_add(o)).map(|ptr| {
			Self {
				ptr,
				_marker: Default::default(),
			}
		})
	}

	/// Create a null pointer.
	pub fn null() -> Self {
		Self::new(0)
	}

	/// Cast this pointer of type `T` to a pointer of type `R`.
	pub fn cast<R: PointerType>(self) -> Pointer<R> {
		Pointer::new(self.ptr)
	}
}

impl<T: PointerType> From<Pointer<T>> for u32 {
	fn from(ptr: Pointer<T>) -> Self {
		ptr.ptr
	}
}

impl<T: PointerType> From<Pointer<T>> for usize {
	fn from(ptr: Pointer<T>) -> Self {
		ptr.ptr as _
	}
}

impl<T: PointerType> IntoValue for Pointer<T> {
	const VALUE_TYPE: ValueType = ValueType::I32;
	fn into_value(self) -> Value { Value::I32(self.ptr as _) }
}

impl<T: PointerType> TryFromValue for Pointer<T> {
	fn try_from_value(val: Value) -> Option<Self> {
		match val {
			Value::I32(val) => Some(Self::new(val as _)),
			_ => None,
		}
	}
}

/// The word size used in wasm. Normally known as `usize` in Rust.
pub type WordSize = u32;

/// The Signature of a function
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Signature {
	/// The arguments of a function.
	pub args: Cow<'static, [ValueType]>,
	/// The optional return value of a function.
	pub return_value: Option<ValueType>,
}

impl Signature {
	/// Create a new instance of `Signature`.
	pub fn new<T: Into<Cow<'static, [ValueType]>>>(args: T, return_value: Option<ValueType>) -> Self {
		Self {
			args: args.into(),
			return_value,
		}
	}

	/// Create a new instance of `Signature` with the given `args` and without any return value.
	pub fn new_with_args<T: Into<Cow<'static, [ValueType]>>>(args: T) -> Self {
		Self {
			args: args.into(),
			return_value: None,
		}
	}

}

/// Something that provides a function implementation on the host for a wasm function.
pub trait Function {
	/// Returns the name of this function.
	fn name(&self) -> &str;
	/// Returns the signature of this function.
	fn signature(&self) -> Signature;
	/// Execute this function with the given arguments.
	fn execute(
		&self,
		context: &mut dyn FunctionContext,
		args: &mut dyn Iterator<Item = Value>,
	) -> Result<Option<Value>>;
}

/// Context used by `Function` to interact with the allocator and the memory of the wasm instance.
pub trait FunctionContext {
	/// Read memory from `address` into a vector.
	fn read_memory(&self, address: Pointer<u8>, size: WordSize) -> Result<Vec<u8>> {
		let mut vec = Vec::with_capacity(size as usize);
		vec.resize(size as usize, 0);
		self.read_memory_into(address, &mut vec)?;
		Ok(vec)
	}
	/// Read memory into the given `dest` buffer from `address`.
	fn read_memory_into(&self, address: Pointer<u8>, dest: &mut [u8]) -> Result<()>;
	/// Write the given data at `address` into the memory.
	fn write_memory(&mut self, address: Pointer<u8>, data: &[u8]) -> Result<()>;
	/// Allocate a memory instance of `size` bytes.
	fn allocate_memory(&mut self, size: WordSize) -> Result<Pointer<u8>>;
	/// Deallocate a given memory instance.
	fn deallocate_memory(&mut self, ptr: Pointer<u8>) -> Result<()>;
	/// Provides access to the sandbox.
	fn sandbox(&mut self) -> &mut dyn Sandbox;
}

/// Sandbox memory identifier.
pub type MemoryId = u32;

/// Something that provides access to the sandbox.
pub trait Sandbox {
	/// Get sandbox memory from the `memory_id` instance at `offset` into the given buffer.
	fn memory_get(
		&mut self,
		memory_id: MemoryId,
		offset: WordSize,
		buf_ptr: Pointer<u8>,
		buf_len: WordSize,
	) -> Result<u32>;
	/// Set sandbox memory from the given value.
	fn memory_set(
		&mut self,
		memory_id: MemoryId,
		offset: WordSize,
		val_ptr: Pointer<u8>,
		val_len: WordSize,
	) -> Result<u32>;
	/// Delete a memory instance.
	fn memory_teardown(&mut self, memory_id: MemoryId) -> Result<()>;
	/// Create a new memory instance with the given `initial` size and the `maximum` size.
	/// The size is given in wasm pages.
	fn memory_new(&mut self, initial: u32, maximum: u32) -> Result<MemoryId>;
	/// Invoke an exported function by a name.
	fn invoke(
		&mut self,
		instance_id: u32,
		export_name: &str,
		args: &[u8],
		return_val: Pointer<u8>,
		return_val_len: WordSize,
		state: u32,
	) -> Result<u32>;
	/// Delete a sandbox instance.
	fn instance_teardown(&mut self, instance_id: u32) -> Result<()>;
	/// Create a new sandbox instance.
	fn instance_new(
		&mut self,
		dispatch_thunk_id: u32,
		wasm: &[u8],
		raw_env_def: &[u8],
		state: u32,
	) -> Result<u32>;
}

/// Something that provides implementations for host functions.
pub trait HostFunctions {
	/// Returns all host functions.
	fn functions() -> &'static [&'static dyn Function];
}

/// Something that can be converted into a wasm compatible `Value`.
pub trait IntoValue {
	/// The type of the value in wasm.
	const VALUE_TYPE: ValueType;

	/// Convert `self` into a wasm `Value`.
	fn into_value(self) -> Value;
}

/// Something that can may be created from a wasm `Value`.
pub trait TryFromValue: Sized {
	/// Try to convert the given `Value` into `Self`.
	fn try_from_value(val: Value) -> Option<Self>;
}

macro_rules! impl_into_and_from_value {
	(
		$(
			$type:ty, $( < $gen:ident >, )? $value_variant:ident,
		)*
	) => {
		$(
			impl $( <$gen> )? IntoValue for $type {
				const VALUE_TYPE: ValueType = ValueType::$value_variant;
				fn into_value(self) -> Value { Value::$value_variant(self as _) }
			}

			impl $( <$gen> )? TryFromValue for $type {
				fn try_from_value(val: Value) -> Option<Self> {
					match val {
						Value::$value_variant(val) => Some(val as _),
						_ => None,
					}
				}
			}
		)*
	}
}

impl_into_and_from_value! {
	u32, I32,
	i32, I32,
	u64, I64,
	i64, I64,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pointer_offset_works() {
		let ptr = Pointer::<u32>::null();

		assert_eq!(ptr.offset(10).unwrap(), Pointer::new(40));
		assert_eq!(ptr.offset(32).unwrap(), Pointer::new(128));

		let ptr = Pointer::<u64>::null();

		assert_eq!(ptr.offset(10).unwrap(), Pointer::new(80));
		assert_eq!(ptr.offset(32).unwrap(), Pointer::new(256));
	}
}
