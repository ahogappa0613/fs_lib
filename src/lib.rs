use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use trie_rs::map::TrieBuilder;

// static mut WORKING_DIR: std::sync::OnceLock<&str> = std::sync::OnceLock::new();

static TRIE: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<kompo_storage::Fs>>> =
    std::sync::OnceLock::new();

static mut WORKING_DIR: Option<&CStr> = None;

pub static mut THREAD_CONTEXT: std::sync::OnceLock<
    std::sync::Arc<std::sync::RwLock<std::collections::HashMap<libc::pthread_t, bool>>>,
> = std::sync::OnceLock::new();

type VALUE = u64;
enum Ruby {
    FALSE = 0x00,
    NIL = 0x04,
    TRUE = 0x14,
}
unsafe extern "C" {
    static FILES: libc::c_char;
    static FILES_SIZE: libc::c_int;
    static PATHS: libc::c_char;
    static PATHS_SIZE: libc::c_int;
    static WD: libc::c_char;
    static START_FILE_PATH: libc::c_char;

    static rb_cObject: VALUE;
    fn rb_define_class(name: *const libc::c_char, rb_super: VALUE) -> VALUE;
    // fn rb_string_value_ptr(v: *const VALUE) -> *const libc::c_char;
    fn rb_define_singleton_method(
        object: VALUE,
        name: *const libc::c_char,
        func: unsafe extern "C" fn(v: VALUE, v2: VALUE) -> VALUE,
        argc: libc::c_int,
    );
    fn rb_need_block();
    // fn rb_block_proc() -> VALUE;
    fn rb_ensure(
        b_proc: unsafe extern "C" fn(VALUE) -> VALUE,
        data1: VALUE,
        e_proc: unsafe extern "C" fn(VALUE) -> VALUE,
        data2: VALUE,
    ) -> VALUE;
    fn rb_yield(v: VALUE) -> VALUE;
}

pub unsafe fn open_from_fs(path: *const libc::c_char) -> Option<i32> {
    // let open_path = unsafe { CStr::from_ptr(path) };
    // let search_path = PathBuf::from(open_path.to_str().expect("invalid path"));

    // let search_path = search_path
    //     .iter()
    //     .map(|os_str| os_str.to_str().unwrap())
    //     .collect::<Vec<_>>();
    // let open_path = open_path.to_str().expect("invalid path");
    // let search_path = open_path.split('/').collect::<Vec<_>>();
    // dbg!(&search_path);

    let path = raw_path_to_kompo_path(path);
    let path = path
        .iter()
        .map(|os_str| os_str.as_os_str())
        .collect::<Vec<_>>();

    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    {
        let mut trie = trie.lock().unwrap();

        trie.open(&path)
    }
}

pub unsafe fn open_at_from_fs(path: *const libc::c_char, base_dir_path: &str) -> Option<i32> {
    // let open_at_path = unsafe { CStr::from_ptr(path) };
    // let open_at_path = open_at_path.to_str().expect("invalid path");
    let absolute = true;
    // let search_path = open_at_path.split('/').collect::<Vec<_>>();
    // let search_path = PathBuf::from(open_at_path.to_str().expect("invalid path"));

    // let search_path = search_path
    //     .iter()
    //     .map(|os_str| os_str.to_str().unwrap())
    //     .collect::<Vec<_>>();

    let path = raw_path_to_kompo_path(path);
    let path = path
        .iter()
        .map(|os_str| os_str.as_os_str())
        .collect::<Vec<_>>();

    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    {
        let mut trie = trie.lock().unwrap();

        if absolute {
            trie.open(&path)
        } else {
            trie.open_at(&path)
        }
    }
}

pub fn close_from_fs(fd: i32) -> Option<i32> {
    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    {
        let mut trie = trie.lock().unwrap();

        trie.close(fd)
    }
}

pub unsafe fn stat_from_fs(path: *const libc::c_char, stat: *mut libc::stat) -> Option<i32> {
    let path = raw_path_to_kompo_path(path);
    let path = path
        .iter()
        .map(|os_str| os_str.as_os_str())
        .collect::<Vec<_>>();

    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    {
        let trie = trie.lock().unwrap();
        trie.stat(&path, stat)
    }
}

pub fn fstat_from_fs(fd: i32, stat: *mut libc::stat) -> Option<i32> {
    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    {
        let trie = trie.lock().unwrap();

        trie.fstat(fd, stat)
    }
}

pub fn read_from_fs(fd: i32, buf: *mut libc::c_void, count: libc::size_t) -> Option<isize> {
    let mut buf = unsafe { std::slice::from_raw_parts_mut(buf as *mut u8, count as usize) };

    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    {
        let mut trie = trie.lock().expect("trie is poisoned");

        trie.read(fd, &mut buf)
    }
}

pub fn getcwd_from_fs(buf: *mut libc::c_char, count: libc::size_t) -> Option<*const libc::c_char> {
    let working_dir = unsafe { WORKING_DIR.unwrap() };

    if buf.is_null() {
        if count == 0 {
            let working_directory_path = working_dir.to_bytes().to_vec().into_boxed_slice();
            let ptr = Box::into_raw(working_directory_path);

            Some(ptr as *const libc::c_char)
        } else {
            if working_dir.to_bytes().len() > count as usize {
                None
            } else {
                todo!()
            }
        }
    } else {
        if working_dir.to_bytes().len() > count as usize {
            None
        } else {
            let buf = unsafe { std::slice::from_raw_parts_mut(buf as *mut u8, count as usize) };
            buf.copy_from_slice(working_dir.to_bytes());
            Some(working_dir.to_bytes().as_ptr())
        }
    }
}

pub fn chdir_from_fs(path: *const libc::c_char) -> Option<isize> {
    let changed_path = unsafe { CStr::from_ptr(path) };
    let path = raw_path_to_kompo_path(path);
    let path = path
        .iter()
        .map(|os_str| os_str.as_os_str())
        .collect::<Vec<_>>();

    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    let bool = {
        let trie = trie.lock().expect("trie is poisoned");

        trie.is_exists_dir(&path)
    };

    if bool {
        unsafe {
            WORKING_DIR = Some(changed_path);
            Some(1)
        }
    } else {
        None
    }
}

pub fn fdopendir_from_fs(fd: i32) -> Option<*mut libc::DIR> {
    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    {
        let trie = trie.lock().unwrap();

        match trie.fdopendir(fd) {
            Some(dir) => {
                let dir = Box::new(dir);
                Some(Box::into_raw(dir) as *mut libc::DIR)
            }
            None => None,
        }
    }
}

pub fn readdir_from_fs(dir: *mut libc::DIR) -> Option<*mut libc::dirent> {
    let mut dir = unsafe { Box::from_raw(dir as *mut kompo_storage::FsDir) };
    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    {
        let trie = trie.lock().unwrap();

        match trie.readdir(&mut dir) {
            Some(dirent) => {
                let dirent = Box::new(dirent);
                Some(Box::into_raw(dirent) as *mut libc::dirent)
            }
            None => None,
        }
    }
}

pub fn closedir_from_fs(dir: *mut libc::DIR) -> Option<i32> {
    let mut dir = unsafe { Box::from_raw(dir as *mut kompo_storage::FsDir) };
    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    {
        let mut trie = trie.lock().unwrap();

        trie.closedir(&mut dir)
    }
}

#[no_mangle]
pub unsafe extern "C-unwind" fn get_start_file() -> *const libc::c_char {
    let path = raw_path_to_kompo_path(&START_FILE_PATH);
    let path = path
        .iter()
        .map(|os_str| os_str.as_os_str())
        .collect::<Vec<_>>();

    let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    {
        let trie = trie.lock().expect("trie is poisoned");

        trie.file_read(&path).expect("Not fund start file")
    }
}

#[no_mangle]
pub unsafe extern "C-unwind" fn get_start_file_name() -> *const libc::c_char {
    std::ffi::CStr::from_ptr(&START_FILE_PATH).as_ptr()
}

fn initialize_trie() -> std::sync::Arc<std::sync::Mutex<kompo_storage::Fs<'static>>> {
    let mut builder = TrieBuilder::new();

    let path_slice = unsafe { std::slice::from_raw_parts(&PATHS, PATHS_SIZE as _) };
    let file_slice = unsafe { std::slice::from_raw_parts(&FILES, FILES_SIZE as _) };

    let splited_path_array = path_slice.split_inclusive(|a| *a == b'\0');
    let splited_file_array = file_slice.split_inclusive(|a| *a == b'\0');

    for (path_bytes, file_byte) in splited_path_array.zip(splited_file_array) {
        let path = Path::new(unsafe {
            CStr::from_bytes_with_nul_unchecked(path_bytes)
                .to_str()
                .unwrap()
        });
        let file = unsafe {
            CStr::from_bytes_with_nul_unchecked(file_byte)
                .to_str()
                .unwrap()
                .as_bytes()
        };
        let path = path.iter().collect::<Vec<_>>();
        builder.push(path, file);
    }

    std::sync::Arc::new(std::sync::Mutex::new(kompo_storage::Fs::new(builder)))
}

fn raw_path_to_kompo_path(raw_path: *const libc::c_char) -> Vec<OsString> {
    let path = unsafe { CStr::from_ptr(raw_path) };
    let path = Path::new(path.to_str().expect("invalid path"));

    if path.is_absolute() {
        path.iter()
            .map(|os_str| os_str.to_os_string())
            .collect::<Vec<_>>()
    } else {
        let working_dir = Path::new(unsafe { WORKING_DIR.unwrap() }.to_str().unwrap());

        working_dir
            .join(path)
            .iter()
            .map(|os_str| os_str.to_os_string())
            .collect::<Vec<_>>()
    }
}

unsafe extern "C" fn context_func(_: VALUE, _: VALUE) -> VALUE {
    rb_need_block();

    let binding = std::sync::Arc::clone(
        THREAD_CONTEXT
            .get()
            .expect("not initialized THREAD_CONTEXT"),
    );
    {
        let mut binding = binding.write().expect("THREAD_CONTEXT is posioned");
        binding.insert(libc::pthread_self(), true);
    }

    unsafe extern "C" fn close(_: VALUE) -> VALUE {
        let binding = std::sync::Arc::clone(
            THREAD_CONTEXT
                .get()
                .expect("not initialized THREAD_CONTEXT"),
        );
        {
            let mut binding = binding.write().expect("THREAD_CONTEXT is posioned");
            binding.insert(libc::pthread_self(), false);
        }

        Ruby::NIL as VALUE
    }

    return rb_ensure(rb_yield, Ruby::NIL as VALUE, close, Ruby::NIL as VALUE);
}

unsafe extern "C" fn is_context_func(_: VALUE, _: VALUE) -> VALUE {
    let binding = std::sync::Arc::clone(
        THREAD_CONTEXT
            .get()
            .expect("not initialized THREAD_CONTEXT"),
    );
    {
        let binding = binding.read().expect("THREAD_CONTEXT is posioned");
        if let Some(bool) = binding.get(&libc::pthread_self()) {
            if *bool {
                Ruby::TRUE as VALUE
            } else {
                Ruby::FALSE as VALUE
            }
        } else {
            unreachable!("not found pthread_t")
        }
    }
}

#[no_mangle]
pub unsafe extern "C-unwind" fn Init_kompo_fs() {
    let c_name = CString::new("Kompo").unwrap();
    let context = CString::new("context").unwrap();
    let is_context = CString::new("context?").unwrap();
    let class = rb_define_class(c_name.as_ptr(), rb_cObject);
    rb_define_singleton_method(class, context.as_ptr(), context_func, 0);
    rb_define_singleton_method(class, is_context.as_ptr(), is_context_func, 0);

    WORKING_DIR = Some(unsafe { CStr::from_ptr(&WD) });

    // let trie = std::sync::Arc::clone(&TRIE.get_or_init(initialize_trie));
    // trie.lock().unwrap().entries();
}
