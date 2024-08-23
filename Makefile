
build-sdl:
	cargo build --features sdl

run-player:
	@cargo run --bin player --features sdl,filedb -- /Users/michaelbohn/_w3d/w3d_data #./testdata/test.wlf

test:
	cargo test --features sdl,filedb
