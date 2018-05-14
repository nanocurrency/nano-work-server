# Nano work server

This project is a dedicated work server for [the Nano cryptocurrency](https://nano.org/).

It supports the `work_generate`, `work_cancel`, and `work_validate` commands from the Nano RPC.
For details on these commands, see [the Nano RPC documentation](https://github.com/nanocurrency/raiblocks/wiki/RPC-protocol).

To see available command line options, run `nano-work-server --help`.

## Installation

### OpenCL
Ubuntu:

```
sudo apt install ocl-icd-opencl-dev
```

Fedora:
```
sudo dnf install ocl-icd-devel
```

### Rust
```
curl https://sh.rustup.rs -sSf | sh
```
### Build and run
```
git clone https://github.com/nanocurrency/nano-work-server.git
cd nano-work-server
cargo build --release
cd target/release
./nano-work-server --help
```
