use std::slice;

/// We need the memory helpers from our SDK
/// Since the SDK's alloc/dealloc are #[no_mangle], they will be exported by this crate too
use rustineverything_sdk::alloc;

/// The main entry point for processing text.
/// Signature: (ptr, len) -> u64 (packed ptr and len)
#[no_mangle]
pub unsafe extern "C" fn process_text(ptr: *mut u8, len: usize) -> u64 {
    // 1. Read input string from the host-provided buffer
    let input_bytes = slice::from_raw_parts(ptr, len);
    let input_str = String::from_utf8_lossy(input_bytes);

    // 2. Perform business logic
    let result_str = format!("[Plugin: Prefix] {}", input_str);
    let result_bytes = result_str.into_bytes();
    
    // 3. Prepare result buffer
    let result_len = result_bytes.len();
    let result_ptr = alloc(result_len);
    
    // 4. Write result to the newly allocated buffer
    let result_slice = slice::from_raw_parts_mut(result_ptr, result_len);
    result_slice.copy_from_slice(&result_bytes);

    // 5. Pack ptr and len into u64
    // High 32 bits: ptr, Low 32 bits: len
    ((result_ptr as u64) << 32) | (result_len as u64)
}
