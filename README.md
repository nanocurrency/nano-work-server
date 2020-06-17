# Nano work server

![Build](https://github.com/nanocurrency/nano-work-server/workflows/Build/badge.svg)

This project is a dedicated work server for [the Nano cryptocurrency](https://nano.org/). See the [documentation](https://docs.nano.org/integration-guides/work-generation/) for details on work generation and the current network difficulty.

**nano-work-server** supports the `work_generate`, `work_cancel`, and `work_validate` commands from the Nano RPC.
For details on these commands, see [the Nano RPC documentation](https://docs.nano.org/commands/rpc-protocol/).

To see available command line options, run `nano-work-server --help`.

To use with the beta network (lower work difficulty), give the flag `--beta`. Working with both networks simultaneously is not supported.

If using more than one work peer, give the flag `--shuffle`. This makes it so that the next request is picked randomly instead of sequentially, which leads to more efficient work generation with multiple peers, especially when they are not in the same network.

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

Windows:
- AMD GPU: [OCL-SDK](https://github.com/GPUOpen-LibrariesAndSDKs/OCL-SDK/releases/)
- Nvidia GPU: [CUDA Toolkit](https://developer.nvidia.com/cuda-toolkit)

### Rust

Linux:

```
curl https://sh.rustup.rs -sSf | sh
```

Windows: follow instructions in https://www.rust-lang.org/tools/install

### Build

```bash
git clone https://github.com/nanocurrency/nano-work-server.git
cd nano-work-server
```

Linux:

```bash
cargo build --release
```

Windows:

```bash
cargo rustc --release
```

Depending on your system configuration, it may be necessary to link against OpenCL explicitly by adding `-- -l OpenCL -L "/path/to/opencl.lib"`

## Using

`nano-work-server --help`

_Note_ difficulty values may be outdated in these examples.

- `work_generate` example:

    ```json
    {
        "action": "work_generate",
        "hash": "718CC2121C3E641059BC1C2CFC45666C99E8AE922F7A807B7D07B62C995D79E2",
        "difficulty": "ffffffc000000000",
        "multiplier": "1.0" // overrides difficulty
    }
    ```
    Response:

    ```json
    {
        "work": "2bf29ef00786a6bc",
        "difficulty": "ffffffd21c3933f4",
        "multiplier": "1.3946469"        
    }
    ```


- `work_validate` example:

    ```json
    {
        "action": "work_validate",
        "hash": "718CC2121C3E641059BC1C2CFC45666C99E8AE922F7A807B7D07B62C995D79E2",
        "work": "2bf29ef00786a6bc",
        "difficulty": "ffffffc000000000",
        "multiplier": "1.0" // overrides difficulty
    }
    ```
    Response:

    ```json
    {
        "valid": "1",
        "difficulty": "ffffffd21c3933f4",
        "multiplier": "1.3946469"
    }
    ```

- `work_cancel` example:
    ```json
    {
        "action": "work_cancel",
        "hash": "718CC2121C3E641059BC1C2CFC45666C99E8AE922F7A807B7D07B62C995D79E2"
    }
    ```
    Response:

    ```json
    {
    }
    ```
