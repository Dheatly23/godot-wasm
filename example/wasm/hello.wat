(module
    ;; Example addition with WASM
    (func (export "add") (param i64 i64) (result i64)
        local.get 0
        local.get 1
        i64.add
    )
    (func $ackermann (param i64 i64) (result i64)
        block
            local.get 0
            i64.eqz
            br_if 0
            local.get 0
            i64.const 1
            i64.sub
            block (result i64)
                local.get 0
                local.get 1
                i64.const 1
                local.get 1
                i64.eqz
                br_if 0
                i64.sub
                call $ackermann
            end
            return_call $ackermann
        end
        local.get 1
        i64.const 1
        i64.add
    )
    (func (export "test") (param i64 i64 i64)
        block
            local.get 0
            local.get 1
            call $ackermann
            local.get 2
            i64.eq
            br_if 0
            unreachable
        end
    )
)