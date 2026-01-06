# lib

# SDL
build-sdl:
    cargo build --features sdl

test-sdl:
    cargo test --features sdl,catalog

# web
build-web:
    cargo build --features web

test-web:
    cargo test --features web,catalog

# web-worklet
build-web-worklet:
    cargo build --release --target wasm32-unknown-unknown --features web-worklet
    rm -f web/worklet.wasm
    cp target/wasm32-unknown-unknown/release/opl.wasm web/worklet.wasm

# player
build-player:
    @cargo build --release --bin opl-player --features sdl,catalog,player-bin

run-player:
    @cargo run --bin opl-player --features sdl,catalog,player-bin -- ./testdata/test.wlf

# all together
build-all: build-sdl build-web build-player build-web-worklet

test-all: build-all test-sdl test-web

publish:
    cargo publish --features sdl
