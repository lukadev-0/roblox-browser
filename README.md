# roblox-browser

A web browser in Roblox.

## how work?

This uses an HTTP server (written in Rust) which uses the Chrome devtools
protocol (through the
[headless_chrome](https://github.com/rust-headless-chrome/rust-headless-chrome))
crate. It will start a browser, create a tab, and start a screencast. Each frame
of the screencast is split into chunks, we then compare the chunks of the new
frame and old frame. Any chunks that are changed, are sent over the wire using a
custom binary protocol.

On the Roblox side of things, we repeatedly poll the server using long-polling
in order to get new data. We then decode any chunks we get and update the
EditableImage. We also send input events to the server.

## how run?

You can download prebuilt binaries and place files from
[the latest release](https://github.com/lukadev-0/roblox-browser/releases/latest).

All you need to do is run the binary and open the Roblox place in Roblox Studio
and press play!

## how build?

To build the Rust server and the Roblox place from source, you need the
following prerequisites:

- The Rust toolchain (you can get it from [rustup](https://rustup.rs/))
- [Aftman](https://github.com/LPGHatGuy/aftman)

### rust

```sh
cargo build --release
```

The binary will be at `target/release/roblox_browser.exe` or
`target/release/roblox_browser`.

### roblox

```sh
# install necessary tools
aftman install

# run build
lune run build-roblox
```

The `rbxl` file will be at `roblox/build/browser.rbxl`.
