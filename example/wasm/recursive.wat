(module
    ;; Example recursive call to host
    (func $host_recurse (import "host" "recurse") (param i64 i64) (result i64))
    (func (export "recurse") (param i64 i64) (result i64)
        local.get 0
        i64.const 1
        i64.sub
        local.get 1
        local.get 0
        i64.const 0
        i64.le_s
        if (param i64) (result i64)
            return
        end
        local.get 0
        i64.add
        call $host_recurse
    )
)
