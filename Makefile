# lib

build-sdl:
	cargo build --features sdl

test:
	cargo test --features sdl,catalog

# player
run-player:
	@cargo run --bin opl-player --features sdl,catalog,player-bin -- /Users/michaelbohn/_w3d/w3d_data #./testdata/test.wlf

# extract
build-extract:
	@cargo build --release --bin opl-extract --features catalog,extract-bin
