extern crate metal;
extern crate objc;

use self::metal::*;
use self::objc::rc::autoreleasepool;
use std::mem;
use byteorder::{ByteOrder, LittleEndian};

static LIBRARY_SRC: &str = include_str!("work.metal");
static mut LIBRARY: Option<Box<Library>> = None;
static mut KERNEL: Option<Box<Function>> = None;

pub type Result<T> = std::result::Result<T, String>;

struct GpuTask {
    difficulty: u64,
    root: [u8;32]
}

pub struct Gpu {
    task: GpuTask,
    device_idx: usize,
    threads: usize,
    threads_per_grid: Option<MTLSize>,
}

impl Gpu {
    pub fn new(
        _platform_idx: usize,
        device_idx: usize,
        threads: usize,
        _local_work_size: Option<usize>,
    ) -> Result<Gpu> {

        let gpu = Gpu {
            task: GpuTask {difficulty: 0, root: [0;32]},
            device_idx: device_idx,
            threads: threads,
            threads_per_grid: None,
        };

        let mut index = 0;
        let devices = Device::all();
        for dev in &devices {
            println!("Metal SDK device {}: {} {}", index, dev.name(), if index == gpu.device_idx {" (selected)"} else {""});
            index+=1;
        }

        Ok(gpu)
    }

    pub fn reset_bufs(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn set_task(&mut self, root: &[u8;32], difficulty: u64) -> Result<()> {
        self.reset_bufs()?;
        self.task.difficulty = difficulty;
        self.task.root = *root;
        Ok(())
    }

    pub fn try(&mut self, out: &mut [u8;8], attempt: u64) -> Result<bool> {
        let mut attempt_bytes = [0u8; 8];
        LittleEndian::write_u64(&mut attempt_bytes, attempt);

        let mut difficulty_bytes = [0u8; 8];
        LittleEndian::write_u64(&mut difficulty_bytes, self.task.difficulty);

        let mut success = false;

        unsafe {
            let device = &Device::all()[self.device_idx];
            let mut initial_run = false;
            if LIBRARY.is_none() {
                initial_run = true;
                let opts = CompileOptions::new();
                opts.set_language_version(MTLLanguageVersion::V2_2);
                opts.set_fast_math_enabled(false);
                LIBRARY = Some(Box::new(device.new_library_with_source(&LIBRARY_SRC, &opts).unwrap()));
                KERNEL = Some(Box::new(LIBRARY.as_ref().unwrap().get_function("blake2b_attempt", None).unwrap()));

                self.threads_per_grid = Some(MTLSize {
                    width: self.threads as u64,
                    height: 1u64,
                    depth: 1u64
                });
            }

            autoreleasepool(|| {
                let command_queue = device.new_command_queue();
                let pipeline_state_descriptor = ComputePipelineDescriptor::new();
                pipeline_state_descriptor.set_compute_function(Some(&KERNEL.as_ref().unwrap()));

                let pipeline_state = device.new_compute_pipeline_state_with_function(
                    pipeline_state_descriptor.compute_function().unwrap()).unwrap();

                let attempt = device.new_buffer_with_data(mem::transmute(attempt_bytes.as_ptr()), 8, MTLResourceOptions::StorageModeShared);
                let root = device.new_buffer_with_data(mem::transmute(self.task.root.as_ptr()), 32, MTLResourceOptions::StorageModeShared);
                let difficulty = device.new_buffer_with_data(mem::transmute(difficulty_bytes.as_ptr()), 8, MTLResourceOptions::StorageModeShared);
                let result = device.new_buffer(16, MTLResourceOptions::StorageModeShared);
                let command_buffer = command_queue.new_command_buffer();
                let encoder = command_buffer.new_compute_command_encoder();

                encoder.set_compute_pipeline_state(&pipeline_state);
                encoder.set_buffer(0, Some(&attempt), 0);
                encoder.set_buffer(1, Some(&root), 0);
                encoder.set_buffer(2, Some(&difficulty), 0);
                encoder.set_buffer(3, Some(&result), 0);

                // Specify how the GPU access the buffers
                encoder.use_resource(&attempt, MTLResourceUsage::Read);
                encoder.use_resource(&root, MTLResourceUsage::Read);
                encoder.use_resource(&difficulty, MTLResourceUsage::Read);
                encoder.use_resource(&result, MTLResourceUsage::Write);

                let thread_group_count = MTLSize {
                    width: pipeline_state.max_total_threads_per_group(),
                    height: 1,
                    depth: 1,
                };

                if initial_run {
                    println!("Threads per thread group: {}", thread_group_count.width);
                }

                encoder.dispatch_thread_groups(self.threads_per_grid.unwrap(), thread_group_count);
                encoder.end_encoding();
                command_buffer.commit();
                command_buffer.wait_until_completed();

                let ptr = result.contents() as *mut u64;
                let nonce = *ptr.offset(0);

                // Note: actual difficulty available here, not currently needed as the caller recomputes
                // let diff = *ptr.offset(1);

                if nonce != 0 {
                    *out = nonce.to_le_bytes();
                    success = true;
                }
            });
        }

        Ok(success)
    }
}
