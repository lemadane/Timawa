use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

#[repr(C)]
pub struct TalasTask {
    pub value: Arc<Mutex<Option<i64>>>,
    pub error: Arc<Mutex<Option<*const std::ffi::c_char>>>,
    pub handle: Option<thread::JoinHandle<()>>,
}

#[no_mangle]
pub extern "C" fn talas_task_create(val: i64) -> *mut TalasTask {
    let task = Box::new(TalasTask {
        value: Arc::new(Mutex::new(Some(val))),
        error: Arc::new(Mutex::new(None)),
        handle: None,
    });
    Box::into_raw(task)
}

#[no_mangle]
pub extern "C" fn talas_task_await(ptr: *mut TalasTask) -> i64 {
    if !ptr.is_null() {
        unsafe {
            let task = &mut *ptr;
            if let Some(h) = task.handle.take() {
                let _ = h.join();
            }
            if let Ok(v) = task.value.lock() {
                if let Some(val) = *v {
                    return val;
                }
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn talas_task_yield() {
    thread::yield_now();
}

#[repr(C)]
pub struct TalasChannel {
    pub tx: Sender<i64>,
    pub rx: Mutex<Receiver<i64>>,
}

#[no_mangle]
pub extern "C" fn talas_channel_create() -> *mut TalasChannel {
    let (tx, rx) = channel();
    let ch = Box::new(TalasChannel {
        tx,
        rx: Mutex::new(rx),
    });
    Box::into_raw(ch)
}

#[no_mangle]
pub extern "C" fn talas_channel_send(ptr: *mut TalasChannel, val: i64) {
    if !ptr.is_null() {
        unsafe {
            let _ = (*ptr).tx.send(val);
        }
    }
}

#[no_mangle]
pub extern "C" fn talas_channel_recv(ptr: *mut TalasChannel) -> i64 {
    if !ptr.is_null() {
        unsafe {
            if let Ok(rx) = (*ptr).rx.lock() {
                if let Ok(val) = rx.recv() {
                    return val;
                }
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn talas_task_cancel(ptr: *mut TalasTask) -> bool {
    if !ptr.is_null() {
        unsafe {
            let task = &mut *ptr;
            if let Some(_h) = task.handle.take() {
                return true;
            }
        }
    }
    false
}

#[no_mangle]
pub extern "C" fn talas_task_await_timeout_cancel(ptr: *mut TalasTask, _timeout_ms: i64) -> i64 {
    if !ptr.is_null() {
        unsafe {
            let task = &mut *ptr;
            if let Some(h) = task.handle.take() {
                let _ = h.join();
            }
            if let Ok(v) = task.value.lock() {
                if let Some(val) = *v {
                    return val;
                }
            }
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn talas_task_all(list_ptr: *mut i8) -> *mut i8 {
    let tlist = list_ptr as *mut crate::collections::TalasList;
    let result_list = crate::collections::talas_list_create();
    let len = crate::collections::talas_list_len(tlist);
    for i in 0..len {
        let task_ptr = crate::collections::talas_list_get(tlist, i) as *mut TalasTask;
        let val = talas_task_await(task_ptr);
        crate::collections::talas_list_add(result_list, val);
    }
    result_list as *mut i8
}

#[no_mangle]
pub extern "C" fn talas_task_any(list_ptr: *mut i8) -> i64 {
    let tlist = list_ptr as *mut crate::collections::TalasList;
    let len = crate::collections::talas_list_len(tlist);
    if len > 0 {
        let task_ptr = crate::collections::talas_list_get(tlist, 0) as *mut TalasTask;
        return talas_task_await(task_ptr);
    }
    0
}

#[no_mangle]
pub extern "C" fn talas_task_race(list_ptr: *mut i8) -> i64 {
    let tlist = list_ptr as *mut crate::collections::TalasList;
    let len = crate::collections::talas_list_len(tlist);
    if len > 0 {
        let task_ptr = crate::collections::talas_list_get(tlist, 0) as *mut TalasTask;
        return talas_task_await(task_ptr);
    }
    0
}

#[no_mangle]
pub extern "C" fn talas_task_all_settled(list_ptr: *mut i8) -> *mut i8 {
    let tlist = list_ptr as *mut crate::collections::TalasList;
    let len = crate::collections::talas_list_len(tlist);
    for i in 0..len {
        let task_ptr = crate::collections::talas_list_get(tlist, i) as *mut TalasTask;
        let _ = talas_task_await(task_ptr);
    }
    list_ptr
}
