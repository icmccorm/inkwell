use std::{
    ffi::CString,
    fmt::{Display, Formatter},
    fs,
    path::PathBuf,
};

use llvm_sys::miri::MiriErrorTrace;

#[derive(Debug)]
pub struct StackTraceItem {
    pub line: u32,
    pub column: u32,
    pub file: PathBuf,
}

impl Display for StackTraceItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            fs::canonicalize(&self.file)
                .unwrap_or(self.file.clone())
                .to_str()
                .unwrap(),
            self.line,
            self.column
        )
    }
}
impl From<&MiriErrorTrace> for StackTraceItem {
    fn from(error_trace: &MiriErrorTrace) -> Self {
        let file_slice =
            unsafe { std::slice::from_raw_parts(error_trace.file as *const u8, error_trace.file_len as usize) };
        let dir_slice = unsafe {
            std::slice::from_raw_parts(error_trace.directory as *const u8, error_trace.directory_len as usize)
        };
        let file_string = unsafe { CString::from_vec_unchecked(file_slice.to_vec().clone()) };
        let dir_string = unsafe { CString::from_vec_unchecked(dir_slice.to_vec().clone()) };
        let mut dir = PathBuf::new();
        dir.push(dir_string.to_string_lossy().to_string());
        dir.push(file_string.to_string_lossy().to_string());
        Self {
            line: error_trace.line,
            column: error_trace.column,
            file: dir,
        }
    }
}

#[derive(Debug)]
pub struct StackTrace {
    pub inst: Option<String>,
    pub traces: Vec<StackTraceItem>,
}
impl StackTrace {
    pub fn new(inst: Option<String>, traces: &[MiriErrorTrace]) -> Self {
        Self {
            inst,
            traces: traces.iter().map(|t| t.into()).collect(),
        }
    }
}

impl Display for StackTrace {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let trace = self
            .traces
            .iter()
            .map(|t| t.to_string())
            .rev()
            .collect::<Vec<String>>()
            .join("\n");
        if let Some(inst) = &self.inst {
            write!(f, "\n@ {}\n\n{}", inst.trim(), trace)
        } else {
            write!(f, "{}", trace)
        }
    }
}
