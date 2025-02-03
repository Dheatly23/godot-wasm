mod classdb;
mod engine;
mod globalscope;
mod input;
mod input_map;
mod ip;
mod marshalls;
mod project_settings;
mod time;

crate::filter_macro! {interface [
    classdb <classdb> -> "classdb",
    engine <engine> -> "engine",
    input <input> -> "input",
    input_map <input_map> -> "input-map",
    ip <ip> -> "ip",
    marshalls <marshalls> -> "marshalls",
    project_settings <project_settings> -> "project-settings",
    time <time> -> "time",
    globalscope <globalscope> -> "globalscope",
]}
