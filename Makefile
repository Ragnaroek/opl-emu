
run-player:
	cargo run --bin player --features sdl -- testdata/test.wlf

test:
	cargo test --features sdl
