// Copyright 2017-2019 Parity Technologies (UK) Ltd.
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

//! Implementation of a Wasm runtime using the Wasmi interpreter.

use std::{str, mem};
use wasmi::{
	Module, ModuleInstance, MemoryInstance, MemoryRef, TableRef, ImportsBuilder, ModuleRef,
	memory_units::Pages, RuntimeValue::{I32, I64, self},
};
use crate::error::{Error, WasmError};
use codec::{Encode, Decode};
use primitives::{sandbox as sandbox_primitives, traits::Externalities};
use crate::host_interface::SubstrateExternals;
use crate::sandbox;
use crate::allocator;
use crate::wasm_utils::interpret_runtime_api_result;
use crate::wasm_runtime::WasmRuntime;
use log::trace;
use parity_wasm::elements::{deserialize_buffer, DataSegment, Instruction, Module as RawModule};
use wasm_interface::{
	FunctionContext, HostFunctions, Pointer, WordSize, Sandbox, MemoryId, Result as WResult,
};

struct FunctionExecutor {
	sandbox_store: sandbox::Store<wasmi::FuncRef>,
	heap: allocator::FreeingBumpHeapAllocator,
	memory: MemoryRef,
	table: Option<TableRef>,
}

impl FunctionExecutor {
	fn new(m: MemoryRef, heap_base: u32, t: Option<TableRef>) -> Result<Self, Error> {
		Ok(FunctionExecutor {
			sandbox_store: sandbox::Store::new(),
			heap: allocator::FreeingBumpHeapAllocator::new(heap_base),
			memory: m,
			table: t,
		})
	}
}

impl sandbox::SandboxCapabilities for FunctionExecutor {
	type SupervisorFuncRef = wasmi::FuncRef;

	fn store(&self) -> &sandbox::Store<Self::SupervisorFuncRef> {
		&self.sandbox_store
	}
	fn store_mut(&mut self) -> &mut sandbox::Store<Self::SupervisorFuncRef> {
		&mut self.sandbox_store
	}
	fn allocate(&mut self, len: WordSize) -> Result<Pointer<u8>, Error> {
		let heap = &mut self.heap;
		self.memory.with_direct_access_mut(|mem| {
			heap.allocate(mem, len)
		})
	}
	fn deallocate(&mut self, ptr: Pointer<u8>) -> Result<(), Error> {
		let heap = &mut self.heap;
		self.memory.with_direct_access_mut(|mem| {
			heap.deallocate(mem, ptr)
		})
	}
	fn write_memory(&mut self, ptr: Pointer<u8>, data: &[u8]) -> Result<(), Error> {
		self.memory.set(ptr.into(), data).map_err(Into::into)
	}
	fn read_memory(&self, ptr: Pointer<u8>, len: WordSize) -> Result<Vec<u8>, Error> {
		self.memory.get(ptr.into(), len as usize).map_err(Into::into)
	}

	fn invoke(
		&mut self,
		dispatch_thunk: &Self::SupervisorFuncRef,
		invoke_args_ptr: Pointer<u8>,
		invoke_args_len: WordSize,
		state: u32,
		func_idx: sandbox::SupervisorFuncIndex,
	) -> Result<i64, Error>
	{
		let result = wasmi::FuncInstance::invoke(
			dispatch_thunk,
			&[
				RuntimeValue::I32(u32::from(invoke_args_ptr) as i32),
				RuntimeValue::I32(invoke_args_len as i32),
				RuntimeValue::I32(state as i32),
				RuntimeValue::I32(usize::from(func_idx) as i32),
			],
			self,
		);
		match result {
			Ok(Some(RuntimeValue::I64(val))) => Ok(val),
			Ok(_) => return Err("Supervisor function returned unexpected result!".into()),
			Err(err) => Err(Error::Trap(err)),
		}
	}
}

impl FunctionContext for FunctionExecutor {
	fn read_memory_into(&self, address: Pointer<u8>, dest: &mut [u8]) -> WResult<()> {
		self.memory.get_into(address.into(), dest).map_err(|e| e.to_string())
	}

	fn write_memory(&mut self, address: Pointer<u8>, data: &[u8]) -> WResult<()> {
		self.memory.set(address.into(), data).map_err(|e| e.to_string())
	}

	fn allocate_memory(&mut self, size: WordSize) -> WResult<Pointer<u8>> {
		let heap = &mut self.heap;
		self.memory.with_direct_access_mut(|mem| {
			heap.allocate(mem, size).map_err(|e| e.to_string())
		})
	}

	fn deallocate_memory(&mut self, ptr: Pointer<u8>) -> WResult<()> {
		let heap = &mut self.heap;
		self.memory.with_direct_access_mut(|mem| {
			heap.deallocate(mem, ptr).map_err(|e| e.to_string())
		})
	}

	fn sandbox(&mut self) -> &mut dyn Sandbox {
		self
	}
}

impl Sandbox for FunctionExecutor {
	fn memory_get(
		&mut self,
		memory_id: MemoryId,
		offset: WordSize,
		buf_ptr: Pointer<u8>,
		buf_len: WordSize,
	) -> WResult<u32> {
		let sandboxed_memory = self.sandbox_store.memory(memory_id).map_err(|e| e.to_string())?;

		match MemoryInstance::transfer(
			&sandboxed_memory,
			offset as usize,
			&self.memory,
			buf_ptr.into(),
			buf_len as usize,
		) {
			Ok(()) => Ok(sandbox_primitives::ERR_OK),
			Err(_) => Ok(sandbox_primitives::ERR_OUT_OF_BOUNDS),
		}
	}

	fn memory_set(
		&mut self,
		memory_id: MemoryId,
		offset: WordSize,
		val_ptr: Pointer<u8>,
		val_len: WordSize,
	) -> WResult<u32> {
		let sandboxed_memory = self.sandbox_store.memory(memory_id).map_err(|e| e.to_string())?;

		match MemoryInstance::transfer(
			&self.memory,
			val_ptr.into(),
			&sandboxed_memory,
			offset as usize,
			val_len as usize,
		) {
			Ok(()) => Ok(sandbox_primitives::ERR_OK),
			Err(_) => Ok(sandbox_primitives::ERR_OUT_OF_BOUNDS),
		}
	}

	fn memory_teardown(&mut self, memory_id: MemoryId) -> WResult<()> {
		self.sandbox_store.memory_teardown(memory_id).map_err(|e| e.to_string())
	}

	fn memory_new(
		&mut self,
		initial: u32,
		maximum: u32,
	) -> WResult<MemoryId> {
		self.sandbox_store.new_memory(initial, maximum).map_err(|e| e.to_string())
	}

	fn invoke(
		&mut self,
		instance_id: u32,
		export_name: &str,
		args: &[u8],
		return_val: Pointer<u8>,
		return_val_len: WordSize,
		state: u32,
	) -> WResult<u32> {
		trace!(target: "sr-sandbox", "invoke, instance_idx={}", instance_id);

		// Deserialize arguments and convert them into wasmi types.
		let args = Vec::<sandbox_primitives::TypedValue>::decode(&mut &args[..])
			.map_err(|_| "Can't decode serialized arguments for the invocation")?
			.into_iter()
			.map(Into::into)
			.collect::<Vec<_>>();

		let instance = self.sandbox_store.instance(instance_id).map_err(|e| e.to_string())?;
		let result = instance.invoke(export_name, &args, self, state);

		match result {
			Ok(None) => Ok(sandbox_primitives::ERR_OK),
			Ok(Some(val)) => {
				// Serialize return value and write it back into the memory.
				sandbox_primitives::ReturnValue::Value(val.into()).using_encoded(|val| {
					if val.len() > return_val_len as usize {
						Err("Return value buffer is too small")?;
					}
					self.write_memory(return_val, val).map_err(|_| "Return value buffer is OOB")?;
					Ok(sandbox_primitives::ERR_OK)
				})
			}
			Err(_) => Ok(sandbox_primitives::ERR_EXECUTION),
		}
	}

	fn instance_teardown(&mut self, instance_id: u32) -> WResult<()> {
		self.sandbox_store.instance_teardown(instance_id).map_err(|e| e.to_string())
	}

	fn instance_new(
		&mut self,
		dispatch_thunk_id: u32,
		wasm: &[u8],
		raw_env_def: &[u8],
		state: u32,
	) -> WResult<u32> {
		// Extract a dispatch thunk from instance's table by the specified index.
		let dispatch_thunk = {
			let table = self.table.as_ref()
				.ok_or_else(|| "Runtime doesn't have a table; sandbox is unavailable")?;
			table.get(dispatch_thunk_id)
				.map_err(|_| "dispatch_thunk_idx is out of the table bounds")?
				.ok_or_else(|| "dispatch_thunk_idx points on an empty table entry")?
				.clone()
		};

		let instance_idx_or_err_code =
			match sandbox::instantiate(self, dispatch_thunk, wasm, raw_env_def, state) {
				Ok(instance_idx) => instance_idx,
				Err(sandbox::InstantiationError::StartTrapped) =>
					sandbox_primitives::ERR_EXECUTION,
				Err(_) => sandbox_primitives::ERR_MODULE,
			};

		Ok(instance_idx_or_err_code as u32)
	}
}

impl FunctionExecutor {
	fn resolver() -> &'static dyn wasmi::ModuleImportResolver {
		struct Resolver;
		impl wasmi::ModuleImportResolver for Resolver {
			fn resolve_func(&self, name: &str, signature: &wasmi::Signature)
				-> std::result::Result<wasmi::FuncRef, wasmi::Error>
			{
				let signature = wasm_interface::Signature::from(signature);

				if let Some((index, func)) = SubstrateExternals::functions().iter()
					.enumerate()
					.find(|f| name == f.1.name())
				{
					if signature == func.signature() {
						Ok(wasmi::FuncInstance::alloc_host(signature.into(), index))
					} else {
						Err(wasmi::Error::Instantiation(
							format!(
								"Invalid signature for function `{}` expected `{:?}`, got `{:?}`",
								func.name(),
								signature,
								func.signature(),
							)
						))
					}
				} else {
					Err(wasmi::Error::Instantiation(
						format!("Export {} not found", name),
					))
				}
			}
		}
		&Resolver
	}
}

impl wasmi::Externals for FunctionExecutor {
	fn invoke_index(&mut self, index: usize, args: wasmi::RuntimeArgs)
		-> Result<Option<wasmi::RuntimeValue>, wasmi::Trap>
	{
		let mut args = args.as_ref().iter().copied().map(Into::into);
		let function = SubstrateExternals::functions().get(index).ok_or_else(||
			Error::from(
				format!("Could not find host function with index: {}", index),
			)
		)?;

		function.execute(self, &mut args)
			.map_err(|msg| Error::FunctionExecution(function.name().to_string(), msg))
			.map_err(wasmi::Trap::from)
			.map(|v| v.map(Into::into))
	}
}

fn get_mem_instance(module: &ModuleRef) -> Result<MemoryRef, Error> {
	Ok(module
		.export_by_name("memory")
		.ok_or_else(|| Error::InvalidMemoryReference)?
		.as_memory()
		.ok_or_else(|| Error::InvalidMemoryReference)?
		.clone())
}

/// Find the global named `__heap_base` in the given wasm module instance and
/// tries to get its value.
fn get_heap_base(module: &ModuleRef) -> Result<u32, Error> {
	let heap_base_val = module
		.export_by_name("__heap_base")
		.ok_or_else(|| Error::HeapBaseNotFoundOrInvalid)?
		.as_global()
		.ok_or_else(|| Error::HeapBaseNotFoundOrInvalid)?
		.get();

	match heap_base_val {
		wasmi::RuntimeValue::I32(v) => Ok(v as u32),
		_ => Err(Error::HeapBaseNotFoundOrInvalid),
	}
}

/// Call a given method in the given wasm-module runtime.
fn call_in_wasm_module(
	ext: &mut dyn Externalities,
	module_instance: &ModuleRef,
	method: &str,
	data: &[u8],
) -> Result<Vec<u8>, Error> {
	call_in_wasm_module_with_custom_signature(
		ext,
		module_instance,
		method,
		|alloc| {
			let offset = alloc(data)?;
			Ok(vec![I32(offset as i32), I32(data.len() as i32)])
		},
		|res, memory| {
			if let Some(I64(retval)) = res {
				let (ptr, length) = interpret_runtime_api_result(retval);
				memory.get(ptr.into(), length as usize).map_err(|_| Error::Runtime).map(Some)
			} else {
				Ok(None)
			}
		}
	)
}

/// Call a given method in the given wasm-module runtime.
fn call_in_wasm_module_with_custom_signature<
	F: FnOnce(&mut dyn FnMut(&[u8]) -> Result<u32, Error>) -> Result<Vec<RuntimeValue>, Error>,
	FR: FnOnce(Option<RuntimeValue>, &MemoryRef) -> Result<Option<R>, Error>,
	R,
>(
	ext: &mut dyn Externalities,
	module_instance: &ModuleRef,
	method: &str,
	create_parameters: F,
	filter_result: FR,
) -> Result<R, Error> {
	// extract a reference to a linear memory, optional reference to a table
	// and then initialize FunctionExecutor.
	let memory = get_mem_instance(module_instance)?;
	let table: Option<TableRef> = module_instance
		.export_by_name("__indirect_function_table")
		.and_then(|e| e.as_table().cloned());
	let heap_base = get_heap_base(module_instance)?;

	let mut fec = FunctionExecutor::new(
		memory.clone(),
		heap_base,
		table,
	)?;

	let parameters = create_parameters(&mut |data: &[u8]| {
		let offset = fec.allocate_memory(data.len() as u32)?;
		fec.write_memory(offset, data).map(|_| offset.into()).map_err(Into::into)
	})?;

	let result = externalities::set_and_run_with_externalities(
		ext,
		|| module_instance.invoke_export(method, &parameters, &mut fec),
	);

	match result {
		Ok(val) => match filter_result(val, &memory)? {
			Some(val) => Ok(val),
			None => Err(Error::InvalidReturn),
		},
		Err(e) => {
			trace!(
				target: "wasm-executor",
				"Failed to execute code with {} pages",
				memory.current_size().0
			);
			Err(e.into())
		},
	}
}

/// Prepare module instance
fn instantiate_module(
	heap_pages: usize,
	module: &Module,
) -> Result<ModuleRef, Error> {
	// start module instantiation. Don't run 'start' function yet.
	let intermediate_instance = ModuleInstance::new(
		module,
		&ImportsBuilder::new()
			.with_resolver("env", FunctionExecutor::resolver())
	)?;

	// Verify that the module has the heap base global variable.
	let _ = get_heap_base(intermediate_instance.not_started_instance())?;

	// Extract a reference to a linear memory.
	let memory = get_mem_instance(intermediate_instance.not_started_instance())?;
	memory.grow(Pages(heap_pages)).map_err(|_| Error::Runtime)?;

	if intermediate_instance.has_start() {
		// Runtime is not allowed to have the `start` function.
		Err(Error::RuntimeHasStartFn)
	} else {
		Ok(intermediate_instance.assert_no_start())
	}
}

/// A state snapshot of an instance taken just after instantiation.
///
/// It is used for restoring the state of the module after execution.
#[derive(Clone)]
struct StateSnapshot {
	/// The offset and the content of the memory segments that should be used to restore the snapshot
	data_segments: Vec<(u32, Vec<u8>)>,
	/// The list of all global mutable variables of the module in their sequential order.
	global_mut_values: Vec<RuntimeValue>,
	heap_pages: u64,
}

impl StateSnapshot {
	// Returns `None` if instance is not valid.
	fn take(
		module_instance: &ModuleRef,
		data_segments: Vec<DataSegment>,
		heap_pages: u64,
	) -> Option<Self> {
		let prepared_segments = data_segments
			.into_iter()
			.map(|mut segment| {
				// Just replace contents of the segment since the segments will be discarded later
				// anyway.
				let contents = mem::replace(segment.value_mut(), vec![]);

				let init_expr = match segment.offset() {
					Some(offset) => offset.code(),
					// Return if the segment is passive
					None => return None
				};

				// [op, End]
				if init_expr.len() != 2 {
					return None;
				}
				let offset = match init_expr[0] {
					Instruction::I32Const(v) => v as u32,
					Instruction::GetGlobal(idx) => {
						let global_val = module_instance.globals().get(idx as usize)?.get();
						match global_val {
							RuntimeValue::I32(v) => v as u32,
							_ => return None,
						}
					}
					_ => return None,
				};

				Some((offset, contents))
			})
			.collect::<Option<Vec<_>>>()?;

		// Collect all values of mutable globals.
		let global_mut_values = module_instance
			.globals()
			.iter()
			.filter(|g| g.is_mutable())
			.map(|g| g.get())
			.collect();

		Some(Self {
			data_segments: prepared_segments,
			global_mut_values,
			heap_pages,
		})
	}

	/// Reset the runtime instance to the initial version by restoring
	/// the preserved memory and globals.
	///
	/// Returns `Err` if applying the snapshot is failed.
	fn apply(&self, instance: &ModuleRef) -> Result<(), WasmError> {
		let memory = instance
			.export_by_name("memory")
			.ok_or(WasmError::ApplySnapshotFailed)?
			.as_memory()
			.cloned()
			.ok_or(WasmError::ApplySnapshotFailed)?;

		// First, erase the memory and copy the data segments into it.
		memory
			.erase()
			.map_err(|_| WasmError::ApplySnapshotFailed)?;
		for (offset, contents) in &self.data_segments {
			memory
				.set(*offset, contents)
				.map_err(|_| WasmError::ApplySnapshotFailed)?;
		}

		// Second, restore the values of mutable globals.
		for (global_ref, global_val) in instance
			.globals()
			.iter()
			.filter(|g| g.is_mutable())
			.zip(self.global_mut_values.iter())
			{
				// the instance should be the same as used for preserving and
				// we iterate the same way it as we do it for preserving values that means that the
				// types should be the same and all the values are mutable. So no error is expected/
				global_ref
					.set(*global_val)
					.map_err(|_| WasmError::ApplySnapshotFailed)?;
			}
		Ok(())
	}
}

/// A runtime along with its initial state snapshot.
#[derive(Clone)]
pub struct WasmiRuntime {
	/// A wasm module instance.
	instance: ModuleRef,
	/// The snapshot of the instance's state taken just after the instantiation.
	state_snapshot: StateSnapshot,
}

impl WasmiRuntime {
	/// Perform an operation with the clean version of the runtime wasm instance.
	fn with<R, F>(&self, f: F) -> R
		where
			F: FnOnce(&ModuleRef) -> R,
	{
		self.state_snapshot.apply(&self.instance).expect(
			"applying the snapshot can only fail if the passed instance is different
			from the one that was used for creation of the snapshot;
			we use the snapshot that is directly associated with the instance;
			thus the snapshot was created using the instance;
			qed",
		);
		f(&self.instance)
	}
}

impl WasmRuntime for WasmiRuntime {
	fn update_heap_pages(&mut self, heap_pages: u64) -> bool {
		self.state_snapshot.heap_pages == heap_pages
	}

	fn call(&mut self, ext: &mut dyn Externalities, method: &str, data: &[u8])
			-> Result<Vec<u8>, Error>
	{
		self.with(|module| {
			call_in_wasm_module(ext, module, method, data)
		})
	}
}

pub fn create_instance(code: &[u8], heap_pages: u64) -> Result<WasmiRuntime, WasmError> {
	let module = Module::from_buffer(&code).map_err(|_| WasmError::InvalidModule)?;

	// Extract the data segments from the wasm code.
	//
	// A return of this error actually indicates that there is a problem in logic, since
	// we just loaded and validated the `module` above.
	let data_segments = extract_data_segments(&code)?;

	// Instantiate this module.
	let instance = instantiate_module(heap_pages as usize, &module)
		.map_err(|e| WasmError::Instantiation(e.to_string()))?;

	// Take state snapshot before executing anything.
	let state_snapshot = StateSnapshot::take(&instance, data_segments, heap_pages)
		.expect(
			"`take` returns `Err` if the module is not valid;
				we already loaded module above, thus the `Module` is proven to be valid at this point;
				qed
				",
		);

	Ok(WasmiRuntime {
		instance,
		state_snapshot,
	})
}

/// Extract the data segments from the given wasm code.
///
/// Returns `Err` if the given wasm code cannot be deserialized.
fn extract_data_segments(wasm_code: &[u8]) -> Result<Vec<DataSegment>, WasmError> {
	let raw_module: RawModule = deserialize_buffer(wasm_code)
		.map_err(|_| WasmError::CantDeserializeWasm)?;

	let segments = raw_module
		.data_section()
		.map(|ds| ds.entries())
		.unwrap_or(&[])
		.to_vec();
	Ok(segments)
}
