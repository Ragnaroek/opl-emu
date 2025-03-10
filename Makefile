# lib
## SDL
build-sdl:
	cargo build --features sdl

test-sdl:
	cargo test --features sdl,catalog

## web
build-web:
	cargo build --features web

test-web:
	cargo test --features web,catalog

# player
build-player:
	@cargo build --release --bin opl-player --features sdl,catalog,player-bin

run-player:
	@cargo run --bin opl-player --features sdl,catalog,player-bin -- /Users/michaelbohn/_w3d/w3d_data #./testdata/test.wlf

# extract
build-extract:
	@cargo build --release --bin opl-extract --features catalog,extract-bin

#soundcheck_w3d
build-soundcheck-w3d:
	@cargo build --bin soundcheck-w3d --features sdl,catalog,soundcheck-w3d-bin

run-soundcheck-w3d:
	@cargo run --bin soundcheck-w3d --features sdl,catalog,soundcheck-w3d-bin -- 50 --folder /Users/michaelbohn/_w3d/w3d_data #./testdata/test.wl

# all together
build-all: build-sdl build-web build-player build-extract build-soundcheck-w3d

test-all: test-sdl test-web

publish:
	cargo publish --features sdl
