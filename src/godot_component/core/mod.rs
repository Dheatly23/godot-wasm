mod array;
mod callable;
mod dictionary;
mod object;
mod packed_array;
mod primitive;
mod signal;
mod typeis;

crate::filter_macro! {interface [
    core <core_filter> -> "core",
    typeis <typeis> -> "typeis",
    primitive <primitive> -> "primitive",
    byte_array <packed_array::byte_array_filter> -> "byte-array",
    int32_array <packed_array::int32_array_filter> -> "int32-array",
    int64_array <packed_array::int64_array_filter> -> "int64-array",
    float32_array <packed_array::float32_array_filter> -> "float32-array",
    float64_array <packed_array::float64_array_filter> -> "float64-array",
    vector2_array <packed_array::vector2_array_filter> -> "vector2-array",
    vector3_array <packed_array::vector3_array_filter> -> "vector3-array",
    color_array <packed_array::color_array_filter> -> "color-array",
    string_array <packed_array::string_array_filter> -> "string-array",
    array <array> -> "array",
    callable <callable> -> "callable",
    dictionary <dictionary> -> "dictionary",
    object <object> -> "object",
    signal <signal> -> "signal",
]}

mod core_filter {
    crate::filter_macro! {method [
        var_equals -> "var-equals",
        var_hash -> "var-hash",
        var_stringify -> "var-stringify",
    ]}
}
