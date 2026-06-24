#![allow(non_camel_case_types, dead_code)]

pub mod types;
pub mod vm;
pub mod read_args;
pub mod cast;
pub mod jni_interface;
pub mod impl_basic;
pub mod impl_err;
pub mod impl_frame;
pub mod impl_refs;
pub mod impl_method;
pub mod impl_object;
pub mod impl_fields;
pub mod impl_strings;
pub mod impl_arrays;
pub mod impl_registry;
pub mod impl_new;
pub mod impl_misc;
pub mod call_method;
pub mod call_static;
pub mod call_nonvirtual;
pub mod build_native;
pub mod state;
pub mod vm_funcs;
pub mod api;

// Re-export all public types and functions at the crate root
// for backward compatibility with consumers using `use libjnivm_sys::*;`
pub use types::*;
pub use vm::*;
pub use jni_interface::JNINativeInterface;
pub use api::*;
