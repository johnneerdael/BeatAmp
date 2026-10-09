use super::*;
use jni::{
    JNIEnv,
    objects::{JByteBuffer, JClass},
    sys::{jboolean, jint, jlong, jstring},
};
use std::{collections::BTreeMap, sync::OnceLock};

static WORKERS: OnceLock<Mutex<BTreeMap<u64, Arc<AudioWorker>>>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn workers() -> &'static Mutex<BTreeMap<u64, Arc<AudioWorker>>> {
    WORKERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeAudio_create(
    _env: JNIEnv,
    _class: JClass,
    rate: jint,
    channels: jint,
    generation: jlong,
) -> jlong {
    if rate <= 0 || !(1..=2).contains(&channels) || generation < 0 {
        return 0;
    }
    match AudioWorker::new(rate as u32, channels as u16, generation as u64) {
        Ok(worker) => {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            workers()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(id, Arc::new(worker));
            id as jlong
        }
        Err(_) => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeAudio_submit(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    data: JByteBuffer,
    bytes: jint,
    start_frame: jlong,
    end_ns: jlong,
    estimated: jboolean,
) -> jboolean {
    if bytes <= 0 || bytes as usize > SAMPLES * 2 || bytes % 2 != 0 || start_frame < 0 || end_ns < 0
    {
        return 0;
    }
    let Ok(registry) = workers().try_lock() else {
        return 0;
    };
    let Some(worker) = registry.get(&(handle as u64)).cloned() else {
        return 0;
    };
    drop(registry);
    let Ok(capacity) = env.get_direct_buffer_capacity(&data) else {
        return 0;
    };
    if bytes as usize > capacity {
        return 0;
    }
    let Ok(address) = env.get_direct_buffer_address(&data) else {
        return 0;
    };
    // SAFETY: JNI verified buffer capacity. Java does not mutate it until the
    // synchronous submit returns. Byte decoding avoids alignment assumptions.
    let input = unsafe { std::slice::from_raw_parts(address, bytes as usize) };
    let mut decoded = [0i16; SAMPLES];
    for (i, pair) in input.chunks_exact(2).enumerate() {
        decoded[i] = i16::from_le_bytes([pair[0], pair[1]]);
    }
    worker.submit(
        &decoded[..bytes as usize / 2],
        start_frame as u64,
        end_ns as u64,
        estimated != 0,
    ) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeAudio_report(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    let worker = workers()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(handle as u64))
        .cloned();
    let json = worker
        .and_then(|w| serde_json::to_string(&w.report()).ok())
        .unwrap_or_else(|| "{\"error\":\"capture unavailable\"}".into());
    env.new_string(json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_nl_neerdael_presetpoc_NativeAudio_destroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    let worker = workers()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&(handle as u64));
    drop(worker);
}

pub(crate) fn latest_report() -> Option<WorkerReport> {
    let registry = workers().try_lock().ok()?;
    let worker = registry.values().next_back()?.clone();
    drop(registry);
    let report = worker.latest.try_lock().ok()?.clone();
    Some(report)
}
