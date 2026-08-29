use std::os::raw::c_char;
use std::ffi::CStr;

#[repr(C)]
pub struct TalasString {
    pub ptr: *const u8,
    pub len: usize,
}

#[no_mangle]
pub extern "C" fn talas_string_create(c_str_ptr: *const c_char) -> TalasString {
    if c_str_ptr.is_null() {
        return TalasString {
            ptr: std::ptr::null(),
            len: 0,
        };
    }
    let cstr = unsafe { CStr::from_ptr(c_str_ptr) };
    let bytes = cstr.to_bytes();
    let len = bytes.len();

    let layout = match std::alloc::Layout::from_size_align(len + 1, 1) {
        Ok(l) => l,
        Err(_) => return TalasString { ptr: c_str_ptr as *const u8, len },
    };

    let heap_ptr = unsafe { std::alloc::alloc(layout) };
    if !heap_ptr.is_null() {
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), heap_ptr, len);
            *heap_ptr.add(len) = 0;
        }
        TalasString {
            ptr: heap_ptr,
            len,
        }
    } else {
        TalasString {
            ptr: c_str_ptr as *const u8,
            len,
        }
    }
}
