use std::fmt::Write;

#[link(wasm_import_module = "host")]
unsafe extern "C" {
    fn write(ptr: u32, n: u32);
}

fn write_log(s: &str) {
    let ptr = s.as_ptr() as u32;
    let n = s.len() as u32;

    unsafe {
        write(ptr, n);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    let mut s = String::new();
    for i in 1..=30 {
        let s = match (i % 3, i % 5) {
            (0, 0) => "Fizzbuzz",
            (0, _) => "Fizz",
            (_, 0) => "Buzz",
            _ => {
                s.clear();
                write!(&mut s, "{}", i).unwrap();
                &s
            }
        };
        write_log(s);
    }
}
