pub mod context;
pub mod errors;
pub mod fs_isolated;
mod items;
pub mod stdio;

pub mod bindings {
    use crate::errors::StreamError;

    wasmtime::component::bindgen!({
        path: "wit",
        world: "wasi:cli/command",
        tracing: false,
        async: false,
        ownership: Borrowing {
            duplicate_if_necessary: false
        },
        trappable_imports: true,
        trappable_error_type: {
            "wasi:io/streams/stream-error" => StreamError,
            "wasi:filesystem/types/error-code" => StreamError,
        },
    });
}

pub struct NullPollable {
    _p: (),
}

impl NullPollable {
    pub(crate) fn new() -> Self {
        Self { _p: () }
    }
}
