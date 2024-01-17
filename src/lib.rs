use std::{
    cell::OnceCell,
    ffi::{c_char, c_int, c_long, c_void, CStr, CString},
    path::PathBuf,
    str::FromStr,
};

type VALUE = u64;

extern "C" {
    static FS: u8;
    static FS_SIZE: u64;

    static LOAD_PATHS: u8;
    static LOAD_PATHS_SIZE: u64;

    static START_PATH: u8;
    static START_PATH_SIZE: u64;

    static rb_cObject: VALUE;

    fn rb_define_class(name: *const c_char, rb_super: VALUE) -> VALUE;
    fn rb_string_value_ptr(v: *const VALUE) -> *const c_char;
    fn rb_define_singleton_method(
        object: VALUE,
        name: *const c_char,
        func: unsafe extern "C" fn(v: VALUE, v2: VALUE) -> VALUE,
        argc: c_int,
    );
    fn rb_str_new(ptr: *const c_char, len: c_long) -> VALUE;
    fn rb_str_new_cstr(ptr: *const c_char) -> VALUE;
    fn rb_raise(exc: VALUE, fmt: *const c_char);
    fn rb_ary_new_from_values(n: c_long, elts: *const VALUE) -> VALUE;
}

enum Ruby {
    FALSE = 0x00,
    NIL = 0x04,
    TRUE = 0x14,
}

static mut FS_DATA: OnceCell<fs_cli::fs::Fs> = OnceCell::new();

fn fs_init() -> fs_cli::fs::Fs {
    let data = unsafe { std::slice::from_raw_parts(&FS, FS_SIZE as usize) };
    postcard::from_bytes(data).unwrap()
}

#[no_mangle]
pub unsafe extern "C" fn get_patch_require() -> *const c_char {
    let data = FS_DATA.get_or_init(fs_init);

    data.get("/root/patch_require.rb").unwrap().as_ptr()
}

#[no_mangle]
unsafe extern "C" fn get_file_from_fs_func(_: VALUE, rb_path: VALUE) -> VALUE {
    let rb_path = rb_string_value_ptr(&rb_path);
    let rb_path = PathBuf::from_str(CStr::from_ptr(rb_path).to_str().unwrap()).unwrap();

    println!("get_file_from_fs: {:?}", rb_path);

    let data = unsafe { FS_DATA.get().unwrap() };

    if let Some(script) = data.get(rb_path.to_str().unwrap()) {
        return unsafe { rb_str_new_cstr(script.as_ptr()) };
    } else {
        return Ruby::NIL as VALUE;

        // if let Some(_ext) = rb_path.extension() {
        //     if let Some(script) = data.get(rb_path.to_str().unwrap()) {
        //         return unsafe { rb_str_new_cstr(script.as_ptr()) };
        //     } else {
        //         // unsafe {
        //         //     rb_raise(
        //         //         rb_eLoadError,
        //         //         format!("cannot load such file -- {}\0", rb_path.display()).as_ptr(),
        //         //     )
        //         // };
        //         return Ruby::NIL as VALUE;
        //     }
        // } else {
        //     for ext in EXT_STR.iter() {
        //         if let Some(script) = data.get(rb_path.with_extension(ext).to_str().unwrap()) {
        //             return unsafe { rb_str_new_cstr(script.as_ptr()) };
        //         }
        //     }

        //     // unsafe {
        //     //     rb_raise(
        //     //         rb_eLoadError,
        //     //         format!("cannot load such file -- {}\0", rb_path.display()).as_ptr(),
        //     //     )
        //     // };
        //     return Ruby::NIL as VALUE;
    }
}

#[no_mangle]
unsafe extern "C" fn get_start_file_name_func(_: VALUE, _: VALUE) -> VALUE {
    rb_str_new(&START_PATH, START_PATH_SIZE as i64)
}

#[no_mangle]
unsafe extern "C" fn get_start_file_script_func(_: VALUE, _: VALUE) -> VALUE {
    let data = FS_DATA.get_or_init(fs_init);
    let data = data
        .get(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
            &START_PATH,
            START_PATH_SIZE as usize,
        )))
        .unwrap();

    rb_str_new(data.as_ptr(), data.len() as i64)
}

#[no_mangle]
unsafe extern "C" fn get_load_paths_func(_: VALUE, _: VALUE) -> VALUE {
    let data = unsafe {
        String::from_utf8_lossy(std::slice::from_raw_parts(
            &LOAD_PATHS,
            LOAD_PATHS_SIZE as usize,
        ))
        .to_string()
    };

    let paths: Vec<VALUE> = data
        .split(|str| str == ',')
        .map(|path| unsafe { rb_str_new(path.as_ptr(), path.len() as i64) })
        .collect();

    unsafe { rb_ary_new_from_values(paths.len() as c_long, paths.as_ptr()) }
}

#[no_mangle]
pub unsafe extern "C" fn Init_patch_require() {
    let c_name = CString::new("Fs").unwrap();
    let get_start_file_script = CString::new("get_start_file_script").unwrap();
    let get_start_file_name = CString::new("get_start_file_name").unwrap();
    let get_load_paths = CString::new("get_load_paths").unwrap();

    let get_file_from_fs = CString::new("get_file_from_fs").unwrap();

    unsafe {
        let class = rb_define_class(c_name.as_ptr(), rb_cObject);
        rb_define_singleton_method(
            class,
            get_start_file_name.as_ptr(),
            get_start_file_name_func,
            0,
        );

        rb_define_singleton_method(
            class,
            get_start_file_script.as_ptr(),
            get_start_file_script_func,
            0,
        );

        rb_define_singleton_method(class, get_load_paths.as_ptr(), get_load_paths_func, 0);

        rb_define_singleton_method(class, get_file_from_fs.as_ptr(), get_file_from_fs_func, 1);
    };
}
