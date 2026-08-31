use std::collections::HashMap;

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

#[repr(C)]
pub struct TalasList {
    pub items: Vec<i64>,
}

#[no_mangle]
pub extern "C" fn talas_list_create() -> *mut TalasList {
    let list = Box::new(TalasList { items: Vec::new() });
    Box::into_raw(list)
}

#[no_mangle]
pub extern "C" fn talas_list_add(ptr: *mut TalasList, val: i64) {
    if !ptr.is_null() {
        unsafe {
            (*ptr).items.push(val);
        }
    }
}

#[no_mangle]
pub extern "C" fn talas_list_get(ptr: *mut TalasList, idx: i64) -> i64 {
    if !ptr.is_null() {
        unsafe {
            let items = &(*ptr).items;
            if idx >= 0 && (idx as usize) < items.len() {
                return items[idx as usize];
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn talas_list_len(ptr: *mut TalasList) -> i64 {
    if !ptr.is_null() {
        unsafe {
            return (*ptr).items.len() as i64;
        }
    }
    0
}

#[repr(C)]
pub struct TalasMap {
    pub entries: HashMap<i64, i64>,
}

#[no_mangle]
pub extern "C" fn talas_map_create() -> *mut TalasMap {
    let map = Box::new(TalasMap { entries: HashMap::new() });
    Box::into_raw(map)
}

#[no_mangle]
pub extern "C" fn talas_map_put(ptr: *mut TalasMap, key: i64, val: i64) {
    if !ptr.is_null() {
        unsafe {
            (*ptr).entries.insert(key, val);
        }
    }
}

#[no_mangle]
pub extern "C" fn talas_map_get(ptr: *mut TalasMap, key: i64) -> i64 {
    if !ptr.is_null() {
        unsafe {
            if let Some(val) = (*ptr).entries.get(&key) {
                return *val;
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn talas_map_len(ptr: *mut TalasMap) -> i64 {
    if !ptr.is_null() {
        unsafe {
            return (*ptr).entries.len() as i64;
        }
    }
    0
}
