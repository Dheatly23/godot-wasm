set shell := ["nu", "-c"]

profile := "debug"

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
output_path := addon_path / "bin" / target_triple

default: deploy-addon

# Invoke cargo build
build package target *args:
  cargo build -p {{package}} --target {{target}} --profile {{build_profile}} {{args}}

# Deploy executable to addon
deploy-addon: (build "godot-wasm" target_triple)
  @mkdir -v {{output_path}}; \
  ls "{{target_path}}" \
  | where name =~ "(lib)?godot_wasm\\.(dll|dylib|so)$" \
  | select name size \
  | rename from \
  | insert to {get from | path dirname -r "{{output_path}}"} \
  | each {|f| \
    print $"Copy from: ($f.from)" $"Copy to: ($f.to)" $"Size: ($f.size)"; \
    cp $f.from $f.to \
  }; null

deploy-example: deploy-addon && deploy-wasm
  cp -r -v ./out/addons ./example

build-wasm:
  @ls ./example/wasm \
  | filter {|f| $f.type == "dir" and $f.name !~ ".cargo$"} \
  | get name \
  | path basename \
  | each {|v| \
    print $"Building ($v)"; \
    cargo build -p $v --target wasm32-unknown-unknown --profile {{build_profile}} --config "./example/wasm/.cargo/config.toml"; null \
  }; null

deploy-wasm: build-wasm
  @let cmds = [[cmd closure]; \
    ["wasm-snip" {|f| ^wasm-snip --snip-rust-panicking-code $f -o $f}] \
    ["wasm-opt" {|f| ^wasm-opt -Oz $f -o $f}] \
  ] | filter {which $in.cmd | is-not-empty}; \
  ls "{{"./target/wasm32-unknown-unknown" / target_profile}}" \
  | where name =~ "\\.wasm$" \
  | select name size \
  | rename from \
  | insert to {get from | path dirname -r "./example/wasm"} \
  | each {|f| \
    print $"Copy from: ($f.from)" $"Copy to: ($f.to)" $"Size: ($f.size)"; \
    cp $f.from $f.to; \
    $cmds | each {|c| \
      print $"Running ($c.cmd)"; \
      do $c.closure $f.to \
    }; \
    print $"Final size: (ls $f.to | $in.0.size)"; \
  }; null
