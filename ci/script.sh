#!/bin/bash

set -ex

main() {
    CARGO_CMD="cargo"
    CARGO_FLAGS=""

    if [ -n "$TARGET" ]; then
        CARGO_FLAGS="$CARGO_FLAG --target $TARGET"
    fi

    if [ -n "$TARGET" ]; then
        CARGO_CMD="cross"
    fi

    if [ -n "$RUSTLS" ]; then
        CARGO_FLAGS="$CARGO_FLAGS --no-default-features --features rustls"
    elif [ -n "$TARGET" ]; then
        CARGO_FLAGS="$CARGO_FLAGS --features vendored-openssl"
    fi

    $CARGO_CMD build $CARGO_FLAGS --all

    if [ ! -z $DISABLE_TESTS ]; then
        return
    fi

    if [ ! -z $NO_EXEC_TESTS ]; then
        $CARGO_CMD test $CARGO_FLAGS --all -- '::tests::'
    else
        $CARGO_CMD test $CARGO_FLAGS --all
    fi

    if [ -n "$DEBIAN_PACKAGING" ]; then
        cargo install cargo-deb
        VARIANT=""
        if [ "$TARGET" = "x86_64-unknown-linux-musl" ]; then
            VARIANT="--variant musl"
        fi
        $CARGO_CMD build $CARGO_FLAGS --all --release
        cargo deb $VARIANT --target "$TARGET" --no-build
    fi
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
