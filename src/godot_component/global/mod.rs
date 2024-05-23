mod classdb;
mod engine;
mod globalscope;

crate::filter_macro! {interface [
    classdb <classdb> -> "classdb",
    engine <engine> -> "engine",
    globalscope <globalscope> -> "globalscope",
]}
