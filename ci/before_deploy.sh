#!/bin/bash
# This script takes care of building your crate and packaging it for release

set -ex

main() {
    local src=$(pwd) \
          stage=

    if [ "$TRAVIS_OS_NAME" = "osx" ]; then
        stage=$(mktemp -d -t tmp)
    else
        stage=$(mktemp -d)
    fi

    test -f Cargo.lock || cargo generate-lockfile

    CARGO_CMD="cargo"
    CARGO_FLAGS=""

    if [ -n "$TARGET" ]; then
        CARGO_FLAG="$CARGO_FLAGS --target $TARGET"
    fi

    if [ -n "$TARGET" ]; then
        CARGO_CMD="cross"
    fi

    if [ -n "$RUSTLS" ]; then
        CARGO_FLAGS="$CARGO_FLAGS --no-default-features --features rustls"
    fi

    $CARGO_CMD rustc $CARGO_FLAGS --bin dness --release -- -C lto

    if [ -n "$DEBIAN_PACKAGING" ]; then
        cargo install cargo-deb
        VARIANT=""
        if [ "$TARGET" = "x86_64-unknown-linux-musl" ]; then
            VARIANT="--variant musl"
        fi
        cargo deb $VARIANT --target "$TARGET" --no-build
        cp target/"$TARGET"/debian/dness*.deb $src/.
    fi

    cp target/$TARGET/release/dness $stage/
    cd $stage
    tar czf $src/$CRATE_NAME-$TRAVIS_TAG-$TARGET.tar.gz *

    cd $src

    rm -rf $stage
}

main
