//! Shared types and wire formats for communication between the memory reader,
//! service and client libraries.

#[allow(
    unused_imports,
    reason = "derive aliases are configured before protocol types are added"
)]
#[macro_use(derive)]
extern crate derive_aliases;

mod derive_alias;
