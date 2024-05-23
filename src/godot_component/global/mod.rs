mod classdb;
mod engine;
mod globalscope;
mod input;

crate::filter_macro! {interface [
    classdb <classdb> -> "classdb",
    engine <engine> -> "engine",
    input <input> -> "input",
    globalscope <globalscope> -> "globalscope",
]}
