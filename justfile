set shell := ["nu", "-c"]

features := ""
profile := "debug"
extra_args := env('BUILD_EXTRA_ARGS', "")

addon_path := "./out/addons/godot_wasm"

build_profile := if profile == "release" { "release" } else { "dev" }
target_profile := if profile == "release" { "release" } else { "debug" }

target_arch := if arch() == "x86" {
  "i686"
} else {
  arch()
}
target_triple := target_arch + if os() == "windows" {
  "-pc-windows-msvc"
} else if os() == "linux" {
  "-unknown-linux-gnu"
} else if os() == "macos" {
  "-apple-darwin"
} else {
  error("Unknown OS " + os())
}

target_path := "./target" / target_triple / target_profile

default: deploy-addon

clippy_lints := "-D warnings"

# Invoke cargo build
[group('Cargo')]
build package target features *args:
  cargo build -p {{package}} --target {{target}} --profile {{build_profile}} -F {{quote(features)}} {{args}}

# Invoke cargo fmt
[group('Cargo')]
fmt *args:
  cargo fmt {{args}}

# Invoke cargo check
[group('Cargo')]
check *args:
  cargo check {{args}}

# Invoke cargo clippy
[group('Cargo')]
clippy *args:
  cargo clippy {{args}} -- {{clippy_lints}}

# Deploy executable to addon
[group('Deploy')]
deploy-addon: (build "godot-wasm" target_triple features extra_args)
  @const target_dir = "{{addon_path / "bin" / target_triple}}"; \
  ls "{{target_path}}" \
  | select name size \
  | rename from \
  | insert file { get from | path basename } \
  | where file =~ "^(lib)?godot_wasm\\.(dll|pdb|dylib|so)$" \
  | insert to { \
    get file \
    | path dirname -r $target_dir \
  } \
  | each {|f| \
    print $"Copy from: ($f.from)" $"Copy to: ($f.to)" $"Size: ($f.size)"; \
    mkdir $target_dir; \
    cp $f.from $f.to \
  } | ignore

# Deploy example
[group('Deploy')]
[group('Example')]
deploy-example: deploy-addon && deploy-wasm
  cp -r -v ./out/addons ./example

# Deploy example without WASM compile
[group('Deploy')]
[group('Example')]
deploy-example-nowasm: deploy-addon
  cp -r -v ./out/addons/godot_wasm/bin ./example/addons/godot_wasm

# Build WASM example code
[group('Example')]
build-wasm:
  @ls ./example/wasm \
  | update name {path basename} \
  | where type == "dir" and name != ".cargo" \
  | get name \
  | each {|v| \
    print $"Building ($v)"; \
    cargo build -p $v --target wasm32-unknown-unknown --profile {{build_profile}} --config "./example/wasm/.cargo/config.toml" \
  } | ignore

# Deploy WASM example code
[group('Deploy')]
[group('Example')]
deploy-wasm: build-wasm
  @let cmds = [[cmd closure]; \
    ["wasm-snip" {|f| ^wasm-snip --snip-rust-panicking-code $f -o $f}] \
    ["wasm-opt" {|f| ^wasm-opt -Oz $f -o $f}] \
  ] | filter {which $in.cmd | is-not-empty}; \
  ls "{{"./target/wasm32-unknown-unknown" / target_profile}}" \
  | where ($it.name | str ends-with ".wasm") \
  | select name size \
  | rename from \
  | insert to {$in.from | path dirname -r "./example/wasm"} \
  | each {|f| \
    print $"Copy from: ($f.from)" $"Copy to: ($f.to)" $"Size: ($f.size)"; \
    cp $f.from $f.to; \
    $cmds | each {|c| \
      print $"Running ($c.cmd)"; \
      do $c.closure $f.to \
    }; \
    print $"Final size: (ls $f.to | $in.0.size)"; \
  } | ignore

# Check compilation with multiple configs
[group('Checks')]
compile-test: (fmt "--all" "--check") (check) (clippy) (check "--all-features") (clippy "--all-features") (check "--no-default-features") (clippy "--no-default-features")

# Check WASM example code
[group('Checks')]
[group('Example')]
check-wasm fix="false":
  @ls ./example/wasm \
  | update name {path basename} \
  | where type == "dir" and name != ".cargo" \
  | get name \
  | each {|v| \
    print $"Building ($v)"; \
    cargo clippy -p $v --target wasm32-unknown-unknown --profile {{build_profile}} --config "./example/wasm/.cargo/config.toml" {{ if fix == "true" { "--fix --allow-dirty --allow-staged" } else { "" } }} \
  } | ignore
