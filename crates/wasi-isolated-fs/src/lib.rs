pub mod clock;
pub mod context;
pub mod errors;
pub mod fs_host;
pub mod fs_isolated;
mod items;
mod preview1;
pub mod stdio;
mod wasi;

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
        errors: { errno => trappable Error },
    });

    impl wiggle::GuestErrorType for types::Errno {
        fn success() -> Self {
            Self::Success
        }
    }

    impl From<anyhow::Error> for types::Error {
        fn from(v: anyhow::Error) -> Self {
            Self::trap(v)
        }
    }

    impl From<wasi::filesystem::types::ErrorCode> for types::Errno {
        fn from(code: wasi::filesystem::types::ErrorCode) -> Self {
            match code {
                wasi::filesystem::types::ErrorCode::Access => types::Errno::Acces,
                wasi::filesystem::types::ErrorCode::WouldBlock => types::Errno::Again,
                wasi::filesystem::types::ErrorCode::Already => types::Errno::Already,
                wasi::filesystem::types::ErrorCode::BadDescriptor => types::Errno::Badf,
                wasi::filesystem::types::ErrorCode::Busy => types::Errno::Busy,
                wasi::filesystem::types::ErrorCode::Deadlock => types::Errno::Deadlk,
                wasi::filesystem::types::ErrorCode::Quota => types::Errno::Dquot,
                wasi::filesystem::types::ErrorCode::Exist => types::Errno::Exist,
                wasi::filesystem::types::ErrorCode::FileTooLarge => types::Errno::Fbig,
                wasi::filesystem::types::ErrorCode::IllegalByteSequence => types::Errno::Ilseq,
                wasi::filesystem::types::ErrorCode::InProgress => types::Errno::Inprogress,
                wasi::filesystem::types::ErrorCode::Interrupted => types::Errno::Intr,
                wasi::filesystem::types::ErrorCode::Invalid => types::Errno::Inval,
                wasi::filesystem::types::ErrorCode::Io => types::Errno::Io,
                wasi::filesystem::types::ErrorCode::IsDirectory => types::Errno::Isdir,
                wasi::filesystem::types::ErrorCode::Loop => types::Errno::Loop,
                wasi::filesystem::types::ErrorCode::TooManyLinks => types::Errno::Mlink,
                wasi::filesystem::types::ErrorCode::MessageSize => types::Errno::Msgsize,
                wasi::filesystem::types::ErrorCode::NameTooLong => types::Errno::Nametoolong,
                wasi::filesystem::types::ErrorCode::NoDevice => types::Errno::Nodev,
                wasi::filesystem::types::ErrorCode::NoEntry => types::Errno::Noent,
                wasi::filesystem::types::ErrorCode::NoLock => types::Errno::Nolck,
                wasi::filesystem::types::ErrorCode::InsufficientMemory => types::Errno::Nomem,
                wasi::filesystem::types::ErrorCode::InsufficientSpace => types::Errno::Nospc,
                wasi::filesystem::types::ErrorCode::Unsupported => types::Errno::Notsup,
                wasi::filesystem::types::ErrorCode::NotDirectory => types::Errno::Notdir,
                wasi::filesystem::types::ErrorCode::NotEmpty => types::Errno::Notempty,
                wasi::filesystem::types::ErrorCode::NotRecoverable => types::Errno::Notrecoverable,
                wasi::filesystem::types::ErrorCode::NoTty => types::Errno::Notty,
                wasi::filesystem::types::ErrorCode::NoSuchDevice => types::Errno::Nxio,
                wasi::filesystem::types::ErrorCode::Overflow => types::Errno::Overflow,
                wasi::filesystem::types::ErrorCode::NotPermitted => types::Errno::Perm,
                wasi::filesystem::types::ErrorCode::Pipe => types::Errno::Pipe,
                wasi::filesystem::types::ErrorCode::ReadOnly => types::Errno::Rofs,
                wasi::filesystem::types::ErrorCode::InvalidSeek => types::Errno::Spipe,
                wasi::filesystem::types::ErrorCode::TextFileBusy => types::Errno::Txtbsy,
                wasi::filesystem::types::ErrorCode::CrossDevice => types::Errno::Xdev,
            }
        }
    }

    impl From<std::io::Error> for types::Error {
        fn from(v: std::io::Error) -> Self {
            crate::errors::StreamError::from(v).into()
        }
    }

    impl From<std::num::TryFromIntError> for types::Error {
        fn from(_: std::num::TryFromIntError) -> Self {
            types::Errno::Overflow.into()
        }
    }

    impl From<wiggle::GuestError> for types::Error {
        fn from(err: wiggle::GuestError) -> Self {
            use wiggle::GuestError::*;
            match err {
                InvalidFlagValue { .. } => types::Errno::Inval.into(),
                InvalidEnumValue { .. } => types::Errno::Inval.into(),
                // As per
                // https://github.com/WebAssembly/wasi/blob/main/legacy/tools/witx-docs.md#pointers
                //
                // > If a misaligned pointer is passed to a function, the function
                // > shall trap.
                // >
                // > If an out-of-bounds pointer is passed to a function and the
                // > function needs to dereference it, the function shall trap.
                //
                // so this turns OOB and misalignment errors into traps.
                PtrOverflow { .. } | PtrOutOfBounds { .. } | PtrNotAligned { .. } => {
                    types::Error::trap(err.into())
                }
                PtrBorrowed { .. } => types::Errno::Fault.into(),
                InvalidUtf8 { .. } => types::Errno::Ilseq.into(),
                TryFromIntError { .. } => types::Errno::Overflow.into(),
                SliceLengthsDiffer { .. } => types::Errno::Fault.into(),
                BorrowCheckerOutOfHandles { .. } => types::Errno::Fault.into(),
                InFunc { err, .. } => types::Error::from(*err),
            }
        }
    }
}

pub struct NullPollable {
    _p: (),
}

impl NullPollable {
    pub(crate) fn new() -> Self {
        Self { _p: () }
    }
}
