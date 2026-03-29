default: build

all: test

test: build
	cargo test

build:
	stellar contract build
	@ls -l target/wasm32v1-none/release/*.wasm

build-compressed: build
	@echo "Compressing WASM files..."
	@mkdir -p target/compressed
	@for wasm_file in target/wasm32v1-none/release/*.wasm; do \
		if [ -f "$$wasm_file" ]; then \
			basename=$$(basename "$$wasm_file" .wasm); \
			echo "Optimizing $$basename.wasm..."; \
			wasm-opt -Oz --vacuum --dae --remove-unused-names --remove-unused-types --merge-blocks "$$wasm_file" -o "target/compressed/$$basename.optimized.wasm"; \
			original_size=$$(wc -c < "$$wasm_file"); \
			compressed_size=$$(wc -c < "target/compressed/$$basename.optimized.wasm"); \
			reduction=$$((($$original_size - $$compressed_size) * 100 / $$original_size)); \
			echo "Original: $$original_size bytes, Compressed: $$compressed_size bytes, Reduction: $$reduction%"; \
		fi; \
	done
	@ls -l target/compressed/*.optimized.wasm

fmt:
	cargo fmt --all

clean:
	cargo clean
	rm -rf target/compressed

.PHONY: build-compressed
