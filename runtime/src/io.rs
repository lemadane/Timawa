use std::ffi::CStr;
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn talas_print_i64(value: i64) {
    println!("{}", value);
}

#[no_mangle]
pub extern "C" fn talas_print_f64(value: f64) {
    println!("{}", value);
}

#[no_mangle]
pub extern "C" fn talas_print_bool(value: bool) {
    println!("{}", value);
}

#[no_mangle]
pub extern "C" fn talas_print_str(c_str_ptr: *const c_char) {
    if c_str_ptr.is_null() {
        println!("<null>");
        return;
    }
    let cstr = unsafe { CStr::from_ptr(c_str_ptr) };
    if let Ok(s) = cstr.to_str() {
        println!("{}", s);
    } else {
        println!("<invalid utf8>");
    }
}
