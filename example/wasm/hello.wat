(module
    ;; Example addition with WASM
    (func (export "add") (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.add
    )
)