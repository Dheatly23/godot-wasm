mod classdb;
mod engine;
mod globalscope;
mod input;
mod input_map;
mod ip;

crate::filter_macro! {interface [
    classdb <classdb> -> "classdb",
    engine <engine> -> "engine",
    input <input> -> "input",
    input_map <input_map> -> "input-map",
    ip <ip> -> "ip",
    globalscope <globalscope> -> "globalscope",
]}
