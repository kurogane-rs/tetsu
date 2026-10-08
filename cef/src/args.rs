#[cfg(not(target_os = "windows"))]
use std::ffi::{c_char, CString, OsString};

use crate::*;

#[cfg_attr(target_os = "windows", derive(Clone))]
#[derive(Default)]
pub struct Args {
    /// Arguments `main_args` points into, when this owns them.
    #[cfg(not(target_os = "windows"))]
    owned: Option<OwnedArgv>,
    main_args: MainArgs,
}

/// Arguments as C strings, with C's argv over them.
#[cfg(not(target_os = "windows"))]
struct OwnedArgv {
    source: Vec<CString>,
    // Ends with a null pointer, which argc does not count
    _argv: Vec<*const c_char>,
}

impl Args {
    #[cfg(target_os = "windows")]
    pub fn new() -> Self {
        let main_args = MainArgs {
            instance: tetsu_sys::HINSTANCE(
                unsafe {
                    windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null())
                }
                .cast(),
            ),
        };

        Self { main_args }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn new() -> Self {
        Self::from_source(std::env::args_os().map(c_string).collect())
    }

    /// Returns arguments that own `source`, with C's argv over them.
    #[cfg(not(target_os = "windows"))]
    fn from_source(source: Vec<CString>) -> Self {
        let mut argv = source
            .iter()
            .map(|arg| arg.as_ptr())
            .collect::<Vec<*const c_char>>();
        argv.push(std::ptr::null());
        let main_args = MainArgs {
            argc: source.len() as i32,
            argv: argv.as_ptr() as *mut *mut _,
        };

        Self {
            owned: Some(OwnedArgv {
                source,
                _argv: argv,
            }),
            main_args,
        }
    }

    pub fn as_main_args(&self) -> &MainArgs {
        &self.main_args
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn as_cmd_line(&self) -> Option<CommandLine> {
        let cmd_line = command_line_create()?;
        cmd_line.init_from_argv(self.as_main_args().argc, self.as_main_args().argv.cast());
        Some(cmd_line)
    }

    /// Returns the process's command line as Windows holds it, which CEF
    /// parses itself, quoting included.
    #[cfg(target_os = "windows")]
    pub fn as_cmd_line(&self) -> Option<CommandLine> {
        // SAFETY: the process's command line is a NUL-terminated string that
        // lives as long as the process and is never written
        let command_line = unsafe {
            let command_line = windows_sys::Win32::System::Environment::GetCommandLineW();
            let length = (0..).take_while(|&i| *command_line.add(i) != 0).count();
            std::slice::from_raw_parts(command_line, length)
        };
        cmd_line_from_utf16(command_line)
    }
}

/// Returns a CEF command line parsed from `command_line`, UTF-16 as Windows
/// keeps it.
#[cfg(target_os = "windows")]
fn cmd_line_from_utf16(command_line: &[u16]) -> Option<CommandLine> {
    let cmd_line = command_line_create()?;
    cmd_line.init_from_string(Some(&crate::CefString::from(command_line)));
    Some(cmd_line)
}

/// Returns an OS argument as a C string, its bytes unchanged.
#[cfg(not(target_os = "windows"))]
fn c_string(arg: OsString) -> CString {
    use std::os::unix::ffi::OsStringExt;

    CString::new(arg.into_vec()).expect("an OS argument is a C string, so it holds no NUL")
}

#[cfg(not(target_os = "windows"))]
impl Clone for Args {
    /// Copies owned arguments with an argv of their own; borrowed ones keep
    /// pointing where they did.
    fn clone(&self) -> Self {
        match &self.owned {
            Some(owned) => Self::from_source(owned.source.clone()),
            None => Self {
                owned: None,
                main_args: self.main_args.clone(),
            },
        }
    }
}

impl From<MainArgs> for Args {
    #[cfg(target_os = "windows")]
    fn from(main_args: MainArgs) -> Self {
        Args { main_args }
    }

    #[cfg(not(target_os = "windows"))]
    fn from(main_args: MainArgs) -> Self {
        Args {
            owned: None,
            main_args,
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    fn load_libcef() {
        let cef = sys::find_cef_dir().expect("CEF not found");
        unsafe { sys::load_libcef(&cef.libcef()) }.expect("cannot load libcef");
    }

    #[test]
    fn a_quoted_argument_with_a_space_stays_one_argument() {
        load_libcef();

        let command_line: Vec<u16> = r#"app.exe "a b" --flag"#.encode_utf16().collect();
        let cmd_line = cmd_line_from_utf16(&command_line).expect("a command line");
        let mut arguments = CefStringList::new();
        cmd_line.arguments(Some(&mut arguments));

        let arguments: Vec<String> = arguments.into_iter().collect();
        assert_eq!(arguments, ["a b"]);
        assert_eq!(cmd_line.has_switch(Some(&CefString::from("flag"))), 1);
    }

    #[test]
    fn an_unpaired_surrogate_reaches_cef() {
        load_libcef();

        // `app.exe x` and a lone surrogate, which `std::env::args` refuses
        let mut command_line: Vec<u16> = "app.exe x".encode_utf16().collect();
        command_line.push(0xd800);
        assert!(cmd_line_from_utf16(&command_line).is_some());
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;
    use std::ffi::CStr;

    /// Returns argument `index` of `args` as bytes.
    fn argument(args: &Args, index: usize) -> Vec<u8> {
        let argv = args.as_main_args().argv;
        unsafe { CStr::from_ptr(*argv.add(index)) }
            .to_bytes()
            .to_vec()
    }

    #[test]
    fn argv_ends_with_a_null_pointer_argc_does_not_count() {
        let args = Args::from_source(vec![c_string("app".into()), c_string("--flag".into())]);
        let main_args = args.as_main_args();

        assert_eq!(main_args.argc, 2);
        assert!(unsafe { *main_args.argv.add(2) }.is_null());
        assert_eq!(argument(&args, 1), b"--flag");
    }

    #[test]
    fn a_non_utf8_argument_keeps_its_bytes() {
        use std::os::unix::ffi::OsStringExt;

        let bytes = vec![b'f', 0xff, b'o'];
        let args = Args::from_source(vec![c_string(OsString::from_vec(bytes.clone()))]);

        assert_eq!(argument(&args, 0), bytes);
    }

    #[test]
    fn a_clone_outlives_the_original() {
        let original = Args::from_source(vec![c_string("app".into())]);
        let clone = original.clone();
        drop(original);

        assert_eq!(argument(&clone, 0), b"app");
        assert!(unsafe { *clone.as_main_args().argv.add(1) }.is_null());
    }
}
