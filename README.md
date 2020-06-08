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

Windows:
- AMD GPU: Executable from https://github.com/GPUOpen-LibrariesAndSDKs/OCL-SDK/releases/ will install the required `opencl.lib` in `C:\Program Files (x86)\OCL_SDK_Light\lib\x86_64`
- Nvidia GPU: Through the CUDA toolkit from https://developer.nvidia.com/cuda-toolkit

### Rust

Linux:

```
curl https://sh.rustup.rs -sSf | sh
```

Windows: install using `rustup` from https://www.rust-lang.org/tools/install

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
cargo rustc --release -- -l OpenCL -L "/path/to/opencl.lib"
```

## Using

`nano-work-server --help`

- `work_generate` example:

    ```json
    {
        "action": "work_generate",
        "hash": "718CC2121C3E641059BC1C2CFC45666C99E8AE922F7A807B7D07B62C995D79E2",
        "difficulty": "ffffffc000000000"
    }
    ```
    Response:

    ```json
    {
        "work": "2bf29ef00786a6bc"
    }
    ```


- `work_validate` example:

    ```json
    {
        "action": "work_validate",
        "hash": "718CC2121C3E641059BC1C2CFC45666C99E8AE922F7A807B7D07B62C995D79E2",
        "work": "2bf29ef00786a6bc",
        "difficulty": "ffffffc000000000"
    }
    ```
    Response:

    ```json
    {
        "valid": "1",
        "value": "ffffffd21c3933f4"
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
