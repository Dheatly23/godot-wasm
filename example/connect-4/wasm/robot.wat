;; Stub connect 4 robot

(module
    ;; Some initialization
    (global $width (mut i32) i32.const 0)
    (global $height (mut i32) i32.const 0)
    (global $ix (mut i32) i32.const 0)
    (memory 1 1)

    ;; Initalize robot
    (func
        (export "init")
        (param i64 i64)
        local.get 0
        i32.wrap_i64
        global.set $width
        local.get 1
        i32.wrap_i64
        global.set $height
    )

    ;; Make a move
    (func
        (export "make_move")
        (param $enemy_move i64)
        (result i64)
        (local $i i32)
        local.get $enemy_move
        i32.wrap_i64
        local.tee $i
        local.get $i
        i32.load8_u
        i32.const 1
        i32.add
        i32.store8
        global.get $ix
        local.set $i
        loop $l
            local.get $i
            i32.const 1
            i32.add
            global.get $width
            i32.rem_u
            local.tee $i
            i32.load8_u
            global.get $height
            i32.const 1
            i32.sub
            i32.lt_u
            if
                local.get $i
                local.get $i
                i32.load8_u
                i32.const 1
                i32.add
                i32.store8
                local.get $i
                global.set $ix
                local.get $i
                i64.extend_i32_u
                return
            end
            local.get $i
            global.get $ix
            i32.ne
            br_if $l
        end
        unreachable
    )
)