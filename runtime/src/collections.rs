#[repr(C)]
pub struct TalasArray {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
    pub element_size: usize,
}

#[no_mangle]
pub extern "C" fn talas_array_create(capacity: usize, element_size: usize) -> TalasArray {
    let size = capacity.saturating_mul(element_size);
    let ptr = crate::memory::talas_alloc(size, 8);
    TalasArray {
        data: ptr as *mut u8,
        len: 0,
        capacity,
        element_size,
    }
}
