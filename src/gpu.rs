use ocl;
use ocl::Buffer;
use ocl::Platform;
use ocl::ProQue;
use ocl::Result;
use ocl::builders::DeviceSpecifier;
use ocl::builders::ProgramBuilder;
use ocl::flags::MemFlags;

use byteorder::{ByteOrder, LittleEndian};

pub struct Gpu {
    kernel: ocl::Kernel,
    attempt: Buffer<u8>,
    result: Buffer<u8>,
    root: Buffer<u8>,
}

impl Gpu {
    pub fn new(platform_idx: usize, device_idx: usize, threads: usize) -> Result<Gpu> {
        let prog_bldr = ProgramBuilder::new().src(include_str!("work.cl"));
        let platforms = Platform::list();
        if platforms.len() == 0 {
            return Err("No OpenCL platforms exist (check your drivers and OpenCL setup)".into());
        }
        if platform_idx >= platforms.len() {
            return Err(format!(
                "Platform index {} too large (max {})",
                platform_idx,
                platforms.len() - 1
            ).into());
        }
        let pro_que = ProQue::builder()
            .prog_bldr(prog_bldr)
            .platform(platforms[platform_idx])
            .device(DeviceSpecifier::Indices(vec![device_idx]))
            .dims(1)
            .build()?;

        let device = pro_que.device();
        eprintln!("Initializing GPU: {} {}", device.vendor(), device.name());

        let attempt = Buffer::<u8>::builder()
            .queue(pro_que.queue().clone())
            .flags(MemFlags::new().read_only().host_write_only())
            .dims(8)
            .build()?;
        let result = Buffer::<u8>::builder()
            .queue(pro_que.queue().clone())
            .flags(MemFlags::new().write_only())
            .dims(8)
            .build()?;
        let root = Buffer::<u8>::builder()
            .queue(pro_que.queue().clone())
            .flags(MemFlags::new().read_only().host_write_only())
            .dims(32)
            .build()?;

        let kernel = pro_que
            .create_kernel("raiblocks_work")?
            .gws(threads)
            .arg_buf(&attempt)
            .arg_buf(&result)
            .arg_buf(&root);

        let mut gpu = Gpu {
            kernel,
            attempt,
            result,
            root,
        };
        gpu.reset_bufs()?;
        Ok(gpu)
    }

    pub fn reset_bufs(&mut self) -> Result<()> {
        self.result.write(&[0u8; 8] as &[u8]).enq()?;
        Ok(())
    }

    pub fn set_root(&mut self, root: &[u8]) -> Result<()> {
        self.reset_bufs()?;
        self.root.write(root).enq()?;
        Ok(())
    }

    pub fn try(&mut self, out: &mut [u8], attempt: u64) -> Result<bool> {
        let mut attempt_bytes = [0u8; 8];
        LittleEndian::write_u64(&mut attempt_bytes, attempt);
        self.attempt.write(&attempt_bytes as &[u8]).enq()?;
        debug_assert!(out.iter().all(|&b| b == 0));
        debug_assert!({
            let mut result = [0u8; 8];
            self.result.read(&mut result as &mut [u8]).enq()?;
            result.iter().all(|&b| b == 0)
        });

        unsafe {
            self.kernel.enq()?;
        }

        self.result.read(&mut *out).enq()?;
        let success = !out.iter().all(|&b| b == 0);
        if success {
            self.reset_bufs()?;
        }
        Ok(success)
    }
}
