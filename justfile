default := "ci"

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --workspace

build:
    cargo build --workspace

check:
    cargo fmt --all -- --check
    just lint
    just test

wasm-build:
    rustup target add wasm32-unknown-unknown
    wasm-pack build crates/wasm --target web --out-dir ../../web/src/wasm

web-install:
    pnpm --dir web install

web-lint:
    pnpm --dir web lint

web-build: wasm-build web-install
    pnpm --dir web build

asm:
    for f in tests/programs/*.asm; do nasm -O0 -f bin -o "${f%.asm}.bin" "$f"; done

asm-verify: asm
    git diff --exit-code tests/programs

ci:
    just check
    just asm-verify
    just wasm-build
    just web-lint
    just web-build
