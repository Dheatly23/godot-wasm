# Features

`godot-wasm` contains many features that can be enabled or disabled at compile time.
It can be specified by setting the `features` variable, each feature separated by comma.
To disable default feature, pass `--no-default-features` into `BUILD_EXTRA_ARGS`.
Example command:

```
just features=feat1,feat2,feat3 command
```

## Feature Flags

### Memory Limiter

* Feature: `memory-limiter`
* Default: true

Enables memory limiter to limit memory usage.
By default it's set to unlimited.
Note that the limiter is only approximate and should not be relied to provide hard limit.

### Epoch-based Timeout

* Feature: `epoch-timeout`
* Default: true

Enables epoch-based timeout mechanism to stop possible hang.
Default timeout is 5s.
Default precision is 20ms.

### Object Registry (Legacy)

* Feature: `object-registry-compat`
* Default: true

Enables legacy registry-based Godot value manipulation.
Needs to be enabled via config option.

### Object Registry (New)

* Feature: `object-registry-extern`
* Default: true

Enables extern-based Godot value manipulation.
Needs to be enabled via config option.

### Object Registry

* Feature: `object-registry`
* Default: true

Enables both `object-registry-compat` and `object-registry-extern`.

### WASI 0.1

* Feature: `wasi`
* Default: true

Enables WASI 0.1

### WASI 0.2

* Feature: `wasi-preview2`
* Default: false

Enables WASI 0.2

### Godot Component

* Feature: `godot-component`
* Default: false

Enables component-based Godot value manipulation.

### WASM Threads Proposal

* Feature: `wasm-threads`
* Default: false

Enables WASM threads proposal, allowing use of shared memory,
atomic instruction, and notification.
Note that multi-threading is a separate thing and currently cannot be done inside WASM code.

### Deterministic WASM

* Feature: `deterministic-wasm`
* Default: false

Enables config options that make WASM code execution more equivalent across CPU architecture.
Note that this feature is experimental and may incur performance penalty.

### Precise Timeout

* Feature: `more-precise-timer`
* Default: false

Increase epoch timeout precision to 1ms.
