#![no_main]

use kvstore::KVStore;
use libfuzzer_sys::fuzz_target;
use std::str;
use tokio::runtime::Runtime;

// Fuzzer for the KVStore public methods.
// This requires a running Redis instance at redis://127.0.0.1:6379
//
// The fuzzer will generate arbitrary byte slices, which are then used
// as token, key, and value for the KVStore operations. The goal is to
// find panics or unexpected behavior in the store's implementation when
// handling arbitrary data.

fuzz_target!(|data: &[u8]| {
    // We need a tokio runtime to run our async functions.
    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return,
    };

    rt.block_on(async {
        // Create a new KVStore instance.
        // This will fail if Redis is not running, but the fuzzer will just ignore it
        // and continue with the next input.
        let store = match KVStore::new("redis://127.0.0.1:6379").await {
            Ok(s) => s,
            Err(_) => return,
        };

        // Split the input data into three parts for token, key, and value.
        let (token_data, rest) = data.split_at(data.len().min(36)); // UUID length
        let (key_data, value_data) = rest.split_at(rest.len() / 2);

        // Convert byte slices to &str, ignoring UTF-8 errors.
        // Invalid UTF-8 is a valid input for fuzzing.
        let token = str::from_utf8(token_data).unwrap_or("");
        let key = str::from_utf8(key_data).unwrap_or("");
        let value = str::from_utf8(value_data).unwrap_or("");

        // Fuzz the set, get, and delete operations.
        // We don't care about the results, we're just looking for panics.
        let _ = store.set(token, key, value, None).await;
        let _ = store.get(token, key).await;
        let _ = store.delete(token, key).await;
        let _ = store.list(token, key).await;
        let _ = store.validate_token(token).await;
        let _ = store.health_check().await;
    });
});
