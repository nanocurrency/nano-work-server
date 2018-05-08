extern crate blake2;
extern crate byteorder;
extern crate clap;
extern crate digest;
extern crate futures;
extern crate hex;
extern crate hyper;
extern crate ocl;
extern crate parking_lot;
extern crate rand;
#[macro_use]
extern crate serde_json;

mod gpu;

use std::u64;
use std::collections::VecDeque;
use std::process;
use std::sync::Arc;
use std::thread;
use std::cell::RefCell;
use std::sync::atomic::{self, AtomicBool};

use futures::future::{self, Either, Future};
use futures::sync::oneshot;
use futures::{Async, Stream};

use hyper::server::{Http, Request, Response, Service};
use hyper::StatusCode;

use serde_json::Value;

use rand::Rng;

use blake2::Blake2b;

use digest::{Input, VariableOutput};

use byteorder::{ByteOrder, LittleEndian};

use parking_lot::{Condvar, Mutex};

use gpu::Gpu;

fn work_value(root: [u8; 32], work: [u8; 8]) -> u64 {
    let mut buf = [0u8; 8];
    let mut hasher = Blake2b::new(buf.len()).expect("Unsupported hash length");
    hasher.process(&work);
    hasher.process(&root);
    hasher.variable_result(&mut buf).unwrap();
    LittleEndian::read_u64(&buf as _)
}

#[inline]
fn work_valid(root: [u8; 32], work: [u8; 8]) -> bool {
    work_value(root, work) >= 0xffffffc000000000
}

enum WorkError {
    Canceled,
    Errored,
}

#[derive(Default)]
struct WorkState {
    root: [u8; 32],
    callback: Option<oneshot::Sender<Result<[u8; 8], WorkError>>>,
    task_complete: Arc<AtomicBool>,
    unsuccessful_workers: usize,
    future_work: VecDeque<([u8; 32], oneshot::Sender<Result<[u8; 8], WorkError>>)>,
}

impl WorkState {
    fn set_task(&mut self, cond_var: &Condvar) {
        if self.callback.is_none() {
            if let Some((root, callback)) = self.future_work.pop_front() {
                self.root = root;
                self.callback = Some(callback);
                self.task_complete = Arc::new(AtomicBool::new(false));
                cond_var.notify_all();
            }
        }
    }
}

#[derive(Clone)]
struct RpcService {
    work_state: Arc<(Mutex<WorkState>, Condvar)>,
}

enum RpcCommand {
    WorkGenerate([u8; 32]),
    WorkCancel([u8; 32]),
    WorkValidate([u8; 32], [u8; 8]),
}

enum HexJsonError {
    InvalidHex,
    TooLong,
}

impl RpcService {
    fn generate_work(self, root: [u8; 32]) -> Box<Future<Item = [u8; 8], Error = WorkError>> {
        let (callback_send, callback_recv) = oneshot::channel();
        let callback_send = RefCell::new(Some(callback_send));
        let work_state = self.work_state.clone();
        Box::new(
            future::poll_fn(move || match work_state.0.try_lock() {
                Some(mut state) => {
                    if let Some(callback_send) = callback_send.borrow_mut().take() {
                        state.future_work.push_back((root, callback_send));
                        state.set_task(&work_state.1);
                    }
                    Ok(Async::Ready(()))
                }
                None => Ok(Async::NotReady),
            }).and_then(move |_| callback_recv.map_err(|_| WorkError::Errored))
                .and_then(|x| x),
        )
    }

    fn cancel_work(self, root: [u8; 32]) -> Box<Future<Item = (), Error = ()>> {
        Box::new(future::poll_fn(move || {
            match self.work_state.0.try_lock() {
                Some(mut state) => {
                    let mut i = 0;
                    while i < state.future_work.len() {
                        if state.future_work[i].0 == root {
                            if let Some((_, callback)) = state.future_work.remove(i) {
                                let _ = callback.send(Err(WorkError::Canceled));
                                continue;
                            }
                        }
                        i += 1;
                    }
                    if state.root == root {
                        if let Some(callback) = state.callback.take() {
                            let _ = callback.send(Err(WorkError::Canceled));
                            state.set_task(&self.work_state.1);
                            state.task_complete.store(true, atomic::Ordering::Relaxed);
                        }
                    }
                    Ok(Async::Ready(()))
                }
                None => Ok(Async::NotReady),
            }
        }))
    }

    fn parse_hex_json(value: &Value, out: &mut [u8]) -> Result<(), HexJsonError> {
        let bytes = value
            .as_str()
            .and_then(|s| hex::decode(s).ok())
            .ok_or(HexJsonError::InvalidHex)?;
        if bytes.len() > out.len() {
            return Err(HexJsonError::TooLong);
        }
        for (byte, out) in bytes.iter().rev().zip(out.iter_mut().rev()) {
            *out = *byte;
        }
        Ok(())
    }

    fn parse_hash_json(json: &Value) -> Result<[u8; 32], Value> {
        let root = json.get("hash").ok_or(json!({
            "error": "Failed to deserialize JSON",
            "hint": "Hash field missing",
        }))?;
        let mut out = [0u8; 32];
        Self::parse_hex_json(&root, &mut out).map_err(|err| match err {
            HexJsonError::InvalidHex => json!({
                "error": "Bad block hash",
                "hint": "Expecting a hex string",
            }),
            HexJsonError::TooLong => json!({
                "error": "Bad block hash",
                "hint": "Hash is too long (should be 32 bytes)",
            }),
        })?;
        Ok(out)
    }

    fn parse_work_json(json: &Value) -> Result<[u8; 8], Value> {
        let root = json.get("work").ok_or(json!({
            "error": "Failed to deserialize JSON",
            "hint": "Work field missing",
        }))?;
        let mut out = [0u8; 8];
        Self::parse_hex_json(&root, &mut out).map_err(|err| match err {
            HexJsonError::InvalidHex => json!({
                "error": "Failed to deserialize JSON",
                "hint": "Expecting a hex string for work",
            }),
            HexJsonError::TooLong => json!({
                "error": "Failed to deserialize JSON",
                "hint": "Work is too long (should be 8 bytes)",
            }),
        })?;
        out.reverse();
        Ok(out)
    }

    fn parse_json(json: Value) -> Result<RpcCommand, Value> {
        match json.get("action") {
            None => {
                return Err(json!({
                    "error": "Failed to deserialize JSON",
                    "hint": "Work field missing",
                }))
            }
            Some(action) if action == "work_generate" => {
                Ok(RpcCommand::WorkGenerate(Self::parse_hash_json(&json)?))
            }
            Some(action) if action == "work_cancel" => {
                Ok(RpcCommand::WorkCancel(Self::parse_hash_json(&json)?))
            }
            Some(action) if action == "work_validate" => Ok(RpcCommand::WorkValidate(
                Self::parse_hash_json(&json)?,
                Self::parse_work_json(&json)?,
            )),
            Some(_) => {
                return Err(json!({
                    "error": "Unknown command",
                    "hint": "This isn't rai_node, it's nano-work-server. Supported commands: work_generate, work_cancel, and work_validate."
                }))
            }
        }
    }

    fn process_req(
        self,
        req: Result<Value, serde_json::Error>,
    ) -> Box<Future<Item = (StatusCode, Value), Error = hyper::Error>> {
        let json = match req {
            Ok(json) => json,
            Err(_) => {
                return Box::new(future::ok((
                    StatusCode::BadRequest,
                    json!({
                        "error": "Failed to deserialize JSON",
                    }),
                )));
            }
        };
        let command = match Self::parse_json(json) {
            Ok(r) => r,
            Err(err) => return Box::new(future::ok((StatusCode::BadRequest, err))),
        };
        match command {
            RpcCommand::WorkGenerate(root) => {
                Box::new(self.generate_work(root).then(|res| match res {
                    Ok(work) => {
                        let work: Vec<u8> = work.iter().rev().cloned().collect();
                        Ok((
                            StatusCode::Ok,
                            json!({
                                "work": hex::encode(&work),
                            }),
                        ))
                    }
                    Err(WorkError::Canceled) => Ok((
                        StatusCode::Ok,
                        json!({
                            "error": "Cancelled",
                        }),
                    )),
                    Err(WorkError::Errored) => Ok((
                        StatusCode::Ok,
                        json!({
                            "error": "Work generation failed (see logs for details)",
                        }),
                    )),
                }))
            }
            RpcCommand::WorkCancel(root) => Box::new(
                self.cancel_work(root)
                    .then(|_| Ok((StatusCode::Ok, json!({})))),
            ),
            RpcCommand::WorkValidate(root, work) => {
                let valid = work_valid(root, work);
                Box::new(future::ok((
                    StatusCode::Ok,
                    json!({
                        "valid": if valid { "1" } else { "0" },
                    }),
                )))
            }
        }
    }
}

impl Service for RpcService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        let res_fut = if *req.method() == hyper::Method::Post {
            let self_copy = self.clone();
            Either::A(
                req.body()
                    .concat2()
                    .map(move |chunk| serde_json::from_slice(chunk.as_ref()))
                    .and_then(move |res| self_copy.process_req(res)),
            )
        } else {
            Either::B(future::ok((
                StatusCode::MethodNotAllowed,
                json!({
                    "error": "Can only POST requests",
                }),
            )))
        };
        Box::new(res_fut.map(|(status, body)| {
            let body = body.to_string();
            Response::new()
                .with_header(hyper::header::ContentLength(body.len() as u64))
                .with_header(hyper::header::ContentType::json())
                .with_body(body)
                .with_status(status)
        }))
    }
}

fn main() {
    let args = clap::App::new("Nano work server")
        .version("1.0")
        .author("Lee Bousfield <ljbousfield@gmail.com>")
        .about("Provides a work server for Nano without a full node")
        .arg(
            clap::Arg::with_name("listen_address")
                .short("l")
                .long("listen-address")
                .value_name("ADDR")
                .default_value("[::1]:7076")
                .help("Specifies the address to listen on"),
        )
        .arg(
            clap::Arg::with_name("cpu_threads")
                .short("c")
                .long("cpu-threads")
                .value_name("THREADS")
                .default_value("0")
                .help("Specifies how many CPU threads to use"),
        )
        .arg(
            clap::Arg::with_name("gpu")
                .short("g")
                .long("gpu")
                .value_name("PLATFORM:DEVICE:THREADS")
                .multiple(true)
                .help(
                    "Specifies which GPU(s) to use. THREADS is optional and defaults to 1048576.",
                ),
        )
        .get_matches();
    let listen_addr = args.value_of("listen_address")
        .unwrap()
        .parse()
        .expect("Failed to parse listen address");
    let cpu_threads: usize = args.value_of("cpu_threads")
        .unwrap()
        .parse()
        .expect("Failed to parse CPU threads");
    let gpus: Vec<Gpu> = args.values_of("gpu")
        .map(|x| x.collect())
        .unwrap_or_else(Vec::new)
        .into_iter()
        .map(|s| {
            let mut parts = s.split(':');
            let platform = parts
                .next()
                .expect("GPU string cannot be blank")
                .parse()
                .expect(&format!("Failed to parse GPU platform in string {:?}", s));
            let device = parts
                .next()
                .expect(&format!("GPU string {:?} must have at least one colon", s))
                .parse()
                .expect(&format!("Failed to parse GPU device in string {:?}", s));
            let threads = parts
                .next()
                .unwrap_or("1048576")
                .parse()
                .expect(&format!("Failed to parse GPU threads in string {:?}", s));
            if parts.next().is_some() {
                panic!("Too many colons in GPU string {:?}", s);
            }
            Gpu::new(platform, device, threads)
                .expect(&format!("Failed to create GPU from string {:?}", s))
        })
        .collect();

    let n_workers = gpus.len() + cpu_threads;
    if n_workers == 0 {
        eprintln!("No workers specified. Please use the --gpu or --cpu-threads flags.\nUse --help for more options.");
        process::exit(1);
    }
    let work_state = Arc::new((Mutex::new(WorkState::default()), Condvar::new()));
    let mut worker_handles = Vec::new();
    for _ in 0..cpu_threads {
        let work_state = work_state.clone();
        let mut rng: rand::XorShiftRng = rand::thread_rng().gen();
        let mut root = [0u8; 32];
        let mut task_complete = Arc::new(AtomicBool::new(true));
        let handle = thread::spawn(move || loop {
            if task_complete.load(atomic::Ordering::Relaxed) {
                let mut state = work_state.0.lock();
                while state.callback.is_none() {
                    work_state.1.wait(&mut state);
                }
                root = state.root;
                task_complete = state.task_complete.clone();
            }
            let mut out: [u8; 8] = rng.gen();
            for _ in 0..(1 << 20) {
                if work_valid(root, out) {
                    let mut state = work_state.0.lock();
                    if root == state.root {
                        if let Some(callback) = state.callback.take() {
                            let _ = callback.send(Ok(out));
                            state.set_task(&work_state.1);
                            state.task_complete.store(true, atomic::Ordering::Relaxed);
                        }
                    }
                    break;
                }
                for byte in out.iter_mut() {
                    *byte = byte.wrapping_add(1);
                    if *byte != 0 {
                        // We did not overflow
                        break;
                    }
                }
            }
        });
        worker_handles.push(handle.thread().clone());
    }
    for (gpu_i, mut gpu) in gpus.into_iter().enumerate() {
        let mut failed = false;
        let mut rng: rand::XorShiftRng = rand::thread_rng().gen();
        let mut root = [0u8; 32];
        let work_state = work_state.clone();
        let mut task_complete = Arc::new(AtomicBool::new(true));
        let mut consecutive_gpu_errors = 0;
        let mut consecutive_gpu_invalid_work_errors = 0;
        let handle = thread::spawn(move || loop {
            if failed || task_complete.load(atomic::Ordering::Relaxed) {
                let mut state = work_state.0.lock();
                if root != state.root {
                    failed = false;
                }
                if failed {
                    state.unsuccessful_workers += 1;
                    if state.unsuccessful_workers == n_workers {
                        if let Some(callback) = state.callback.take() {
                            let _ = callback.send(Err(WorkError::Errored));
                            state.set_task(&work_state.1);
                            state.task_complete.store(true, atomic::Ordering::Relaxed);
                        }
                    }
                    work_state.1.wait(&mut state);
                }
                while state.callback.is_none() {
                    work_state.1.wait(&mut state);
                }
                root = state.root;
                task_complete = state.task_complete.clone();
                if failed {
                    state.unsuccessful_workers -= 1;
                }
                if let Err(err) = gpu.set_root(&root) {
                    eprintln!(
                        "Failed to set GPU {}'s task, abandoning it for this work: {:?}",
                        gpu_i, err,
                    );
                    failed = true;
                    continue;
                }
                failed = false;
                consecutive_gpu_errors = 0;
            }
            let attempt = rng.gen();
            let mut out = [0u8; 8];
            match gpu.try(&mut out, attempt) {
                Ok(true) => {
                    if work_valid(root, out) {
                        let mut state = work_state.0.lock();
                        if root == state.root {
                            if let Some(callback) = state.callback.take() {
                                let _ = callback.send(Ok(out));
                                state.set_task(&work_state.1);
                                state.task_complete.store(true, atomic::Ordering::Relaxed);
                            }
                        }
                        consecutive_gpu_errors = 0;
                        consecutive_gpu_invalid_work_errors = 0;
                    } else {
                        eprintln!(
                            "GPU {} returned invalid work {} for root {}",
                            gpu_i,
                            hex::encode(&out),
                            hex::encode_upper(&root),
                        );
                        if consecutive_gpu_invalid_work_errors >= 3 {
                            eprintln!("GPU {} returned invalid work 3 consecutive times, abandoning it for this work", gpu_i);
                            failed = true;
                        } else {
                            consecutive_gpu_errors += 1;
                            consecutive_gpu_invalid_work_errors += 1;
                        }
                    }
                }
                Ok(false) => {
                    consecutive_gpu_errors = 0;
                }
                Err(err) => {
                    eprintln!("Error computing work on GPU {}: {:?}", gpu_i, err);
                    if let Err(err) = gpu.reset_bufs() {
                        eprintln!(
                            "Failed to reset GPU {}'s buffers, abandoning it for this work: {:?}",
                            gpu_i, err,
                        );
                        failed = true;
                    }
                    consecutive_gpu_errors += 1;
                }
            }
            if consecutive_gpu_errors >= 3 {
                eprintln!(
                    "3 consecutive GPU {} errors, abandoning it for this work",
                    gpu_i,
                );
                failed = true;
            }
        });
        worker_handles.push(handle.thread().clone());
    }

    let server = Http::new()
        .bind(&listen_addr, move || {
            Ok(RpcService {
                work_state: work_state.clone(),
            })
        })
        .expect("Failed to bind server");
    server.run().expect("Error running server");
}
