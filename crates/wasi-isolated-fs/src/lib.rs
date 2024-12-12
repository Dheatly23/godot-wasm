mod errors;
pub mod fs_isolated;
pub mod stdio;

pub mod bindings {
    wasmtime::component::bindgen!({
        path: "wit",
        tracing: false,
        async: false,
        ownership: Borrowing {
            duplicate_if_necessary: false
        },
        trappable_imports: true,
    });
}
