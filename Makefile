.PHONY: test

test:
	cargo build --features ai_test --bin aiscript-cli-test && cargo test