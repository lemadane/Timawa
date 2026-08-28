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
    TalasString {
        ptr: bytes.as_ptr(),
        len: bytes.len(),
    }
}
