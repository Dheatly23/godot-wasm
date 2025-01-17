pub mod clock;
pub mod context;
pub mod errors;
pub mod fs_host;
pub mod fs_isolated;
mod items;
mod poll;
pub mod preview1;
pub mod stdio;
mod wasi;

use std::fmt::{Debug, Formatter, Result as FmtResult};

static EMPTY_BUF: [u8; 4096] = [0; 4096];

#[allow(clippy::too_many_arguments)]
pub mod bindings {
    use crate::errors::{NetworkError, StreamError};

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
            "wasi:sockets/network/error-code" => NetworkError,
        },
    });

    wiggle::from_witx!({
        witx: ["$CARGO_MANIFEST_DIR/witx/wasi_snapshot_preview1.witx"],
        errors: { errno => StreamError },
    });

    impl wiggle::GuestErrorType for types::Errno {
        fn success() -> Self {
            Self::Success
        }
    }
}

pub struct NullPollable {
    _p: (),
}

impl Debug for NullPollable {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "NullPollable")
    }
}

impl NullPollable {
    pub(crate) fn new() -> Self {
        Self { _p: () }
    }
}
