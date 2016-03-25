//! An OpenCL program.
use std;
use std::ops::{Deref, DerefMut};
use std::ffi::CString;
use std::io::Read;
use std::fs::File;
use std::path::PathBuf;
use std::collections::HashSet;
use std::convert::Into;

use error::{Result as OclResult, Error as OclError};
use core::{self, Program as ProgramCore, Context as ContextCore,
    ProgramInfo, ProgramInfoResult, ProgramBuildInfo, ProgramBuildInfoResult};
use standard::{Context, Device, DeviceSpecifier};


/// A build option used by ProgramBuilder.
///
/// Strings intended for use either by the compiler as a command line switch
/// or for inclusion in the final build source code.
///
/// A few of the often used variants have constructors for convenience.
///
/// [FIXME] TODO: Explain how each variant is used.
///
/// [FIXME] TODO: Examples.
#[derive(Clone, Debug)]
pub enum BuildOpt {
    CmplrDefine { ident: String, val: String },
    CmplrInclDir { path: String },
    CmplrOther(String),
    IncludeDefine { ident: String, val: String },
    IncludeCore(String),
    IncludeRawEof(String),
}

impl BuildOpt {
    /// Returns a `BuildOpt::CmplrDefine`.
    pub fn cmplr_def<S: Into<String>>(ident: S, val: i32) -> BuildOpt {
        BuildOpt::CmplrDefine {
            ident: ident.into(),
            val: val.to_string(),
        }
    }

    /// Returns a `BuildOpt::CmplrOther`.
    pub fn cmplr_opt<S: Into<String>>(opt: S) -> BuildOpt {
        BuildOpt::CmplrOther(opt.into())
    }

    /// Returns a `BuildOpt::IncludeDefine`.
    pub fn include_def<S: Into<String>>(ident: S, val: String) -> BuildOpt {
        BuildOpt::IncludeDefine {
            ident: ident.into(),
            val: val,
        }
    }
}


/// A builder for `Program`.
#[derive(Clone, Debug)]
pub struct ProgramBuilder {
    options: Vec<BuildOpt>,
    src_file_names: Vec<String>,
    src_files: Vec<PathBuf>,
    // device_idxs: Vec<usize>,
    // devices: Vec<Device>,
    device_spec: Option<DeviceSpecifier>,
    // embedded_kernel_source: Vec<String>,
}

impl ProgramBuilder {
    /// Returns a new, empty, build configuration object.
    pub fn new() -> ProgramBuilder {
        ProgramBuilder {
            options: Vec::with_capacity(64),
            src_file_names: Vec::with_capacity(16),
            src_files: Vec::with_capacity(16),
            // device_idxs: Vec::with_capacity(8),
            // devices: Vec::with_capacity(8),
            device_spec: None,
            // embedded_kernel_source: Vec::with_capacity(32),
        }
    }

    // pub fn with_opts<S: Into<String> + Clone>(options: Vec<BuildOpt>, src_file_names: &[S]
    //         ) -> ProgramBuilder 
    // {
    //     let src_file_names: Vec<String> = src_file_names.iter().map(|s| s.clone().into()).collect();

    //     ProgramBuilder {
    //         options: options,
    //         src_file_names: src_file_names,
    //         // device_idxs: Vec::with_capacity(8),
    //         devices: Vec::with_capacity(8),
    //     }
    // }

    /// Returns a newly built Program.
    ///
    /// TODO: If the context is associated with more than one device,
    /// check that at least one of those devices has been specified. An empty
    /// device list will cause an OpenCL error in that case.
    ///
    /// TODO: Check for duplicate devices in the final device list.
    pub fn build(&self, context: &Context) -> OclResult<Program> {
        // let mut device_list: Vec<Device> = self.devices.iter().map(|d| d.clone()).collect();
        // device_list.extend_from_slice(&context.resolve_wrapping_device_idxs(&self.device_idxs));
        // let device_list = &self.devices;

        let device_list = match self.device_spec {
            Some(ref ds) => try!(ds.to_device_list(context.platform())),
            None => vec![],
        };

        if device_list.len() == 0 {
            return OclError::err("ocl::ProgramBuilder::build: No devices found.");
        }

        Program::new(
            try!(self.get_src_strings().map_err(|e| e.to_string())), 
            try!(self.get_compiler_options().map_err(|e| e.to_string())), 
            context, 
            &device_list[..])
    }

    /// Adds a build option containing a compiler command line definition.
    /// Formatted as `-D {name}={val}`.
    pub fn cmplr_def<S: Into<String>>(mut self, name: S, val: i32) -> ProgramBuilder {
        self.options.push(BuildOpt::cmplr_def(name, val));
        self
    }

    /// Adds a build option containing a core compiler command line parameter. 
    /// Formatted as `{co}` (exact text).
    pub fn cmplr_opt<S: Into<String>>(mut self, co: S) -> ProgramBuilder {
        self.options.push(BuildOpt::cmplr_opt(co));
        self
    }

    /// Pushes pre-created build option to the list.
    pub fn bo(mut self, bo: BuildOpt) -> ProgramBuilder {
        self.options.push(bo);
        self
    }

    // /// Adds a kernel file to the list of included sources.
    // pub fn src_file_name<S: Into<String>>(mut self, file_name: S) -> ProgramBuilder {
    //     self.src_file_names.push(file_name.into());
    //     self
    // }   

    /// Adds a kernel file to the list of included sources.
    pub fn src_file<P: Into<PathBuf>>(mut self, file_path: P) -> ProgramBuilder {
        self.src_files.push(file_path.into());
        self
    }   

    /// Adds text to the included kernel source.
    pub fn src<S: Into<String>>(mut self, src: S) -> ProgramBuilder {
        // self.add_src(src);
        self.options.push(BuildOpt::IncludeRawEof(src.into()));
        self
    }

    // /// Specify which devices this program should be built on using a vector of 
    // /// zero-based device indexes.
    // ///
    // /// # Example
    // ///
    // /// If your system has 4 OpenGL devices and you want to include them all:
    // /// ```
    // /// let program = program::builder()
    // ///     .src(source_str)
    // ///     .device_idxs(vec![0, 1, 2, 3])
    // ///     .build(context);
    // /// ```
    // /// Out of range device indexes will simply round-robin around to 0 and
    // /// count up again (modulo).
    // pub fn device_idxs(mut self, device_idxs: &[usize]) -> ProgramBuilder {
    //     self.device_idxs.extend_from_slice(&device_idxs);
    //     self
    // }

    // /// Specify a list of devices to build this program on. The devices must be 
    // /// associated with the context passed to `::build` later on.
    // pub fn devices<D: AsRef<[Device]>>(mut self, devices: D) -> ProgramBuilder {
    //     self.devices.extend_from_slice(devices.as_ref());
    //     self
    // }

    // /// Specify a list of devices to build this program on. The devices must be 
    // /// associated with the context passed to `::build` later on.
    // pub fn device(mut self, device: Device) -> ProgramBuilder {
    //     self.devices.push(device);
    //     self
    // }

    pub fn devices<D: Into<DeviceSpecifier>>(mut self, device_spec: D) 
            -> ProgramBuilder 
    {
        assert!(self.device_spec.is_none(), "ocl::ProgramBuilder::devices(): Devices already specified");
        self.device_spec = Some(device_spec.into());
        self
    }

    // /// Adds a kernel file to the list of included sources (in place).
    // pub fn add_src_file(&mut self, file_name: String) {
    //     self.src_file_names.push(file_name);
    // }

    // /// Adds text to the included kernel source (in place).
    // pub fn add_src<S: Into<String>>(&mut self, src: S) {
    //     // self.embedded_kernel_source.push(source.into());
    //     self.options.push(BuildOpt::IncludeRawEof(src.into()));
    // }

    // /// Adds a pre-created build option to the list (in place).
    // pub fn add_bo(&mut self, bo: BuildOpt) {
    //     self.options.push(bo);
    // }

    /// Returns a list of kernel file names added for inclusion in the build.
    pub fn get_src_file_names(&self) -> &Vec<String> {
        &self.src_file_names
    }

    // // Returns the list of devices with which this `ProgramBuilder` is
    // // configured to build on.
    // pub fn get_devices(&self) -> &[Device] {
    //     &self.devices[..]
    // }

    // Returns the devices with which this `ProgramBuilder` is configured to
    // build on.
    pub fn get_device_spec(&self) -> &Option<DeviceSpecifier> {
        &self.device_spec
    }

    /// Parses `self.options` for options intended for inclusion at the beginning of 
    /// the final program source and returns them as a list of strings.
    ///
    /// Generally used for #define directives, constants, etc. Normally called from
    /// `::get_src_strings()` but can also be called from anywhere for debugging 
    /// purposes.
    fn get_kernel_includes(&self) -> OclResult<Vec<CString>> {
        let mut strings = Vec::with_capacity(64);
        strings.push(try!(CString::new("\n".as_bytes())));

        for option in self.options.iter() {
            match option {
                &BuildOpt::IncludeDefine { ref ident, ref val } => {
                    strings.push(try!(CString::new(format!("#define {}  {}\n", ident, val).into_bytes())));
                },
                &BuildOpt::IncludeCore(ref text) => {
                    strings.push(try!(CString::new(text.clone().into_bytes())));
                },
                _ => (),
            };

        }

        Ok(strings)
    }

    /// Parses `self.options` for options intended for inclusion at the end of 
    /// the final program source and returns them as a list of strings.
    fn get_kernel_includes_eof(&self) -> OclResult<Vec<CString>> {
        let mut strings = Vec::with_capacity(64);
        strings.push(try!(CString::new("\n".as_bytes())));

        for option in self.options.iter() {
            match option {
                &BuildOpt::IncludeRawEof(ref text) => {
                    strings.push(try!(CString::new(text.clone().into_bytes())));
                },
                _ => (),
            };
        }

        Ok(strings)     
    }

    /// Returns a contatenated string of compiler command line options used when 
    /// building a `Program`.
    pub fn get_compiler_options(&self) -> OclResult<CString> {
        let mut opts: Vec<String> = Vec::with_capacity(64);

        opts.push(" ".to_owned());

        for option in self.options.iter() {         
            match option {
                &BuildOpt::CmplrDefine { ref ident, ref val } => {
                    opts.push(format!("-D{}={}", ident, val))
                },

                &BuildOpt::CmplrInclDir { ref path } => {
                    opts.push(format!("-I{}", path))
                },

                &BuildOpt::CmplrOther(ref s) => {
                    opts.push(s.clone())
                },

                _ => (),    
            }
        }

        CString::new(opts.join(" ").into_bytes()).map_err(|err| OclError::from(err))
    }

    /// Returns the final program source code as a list of strings.
    ///
    /// Order of inclusion:
    /// - includes from `::get_kernel_includes()`
    /// - source from files listed in `self.src_file_names` in reverse order
    /// - core source from `self.embedded_kernel_source`
    pub fn get_src_strings(&self) -> OclResult<Vec<CString>> {
        let mut src_strings: Vec<CString> = Vec::with_capacity(64);
        // let mut src_file_history: HashSet<&String> = HashSet::with_capacity(64);
        let mut src_file_history: HashSet<PathBuf> = HashSet::with_capacity(64);

        src_strings.extend_from_slice(&try!(self.get_kernel_includes()));

        // for kfn in self.get_src_file_names().iter().rev() {
        for srcpath in self.src_files.iter().rev() {
            let mut src_bytes: Vec<u8> = Vec::with_capacity(100000);

            if src_file_history.contains(srcpath) { continue; }
            src_file_history.insert(srcpath.clone());

            // let valid_kfp = Path::new(kfn);
            let mut src_file_handle = try!(File::open(srcpath));

            try!(src_file_handle.read_to_end(&mut src_bytes));
            src_bytes.shrink_to_fit();
            src_strings.push(try!(CString::new(src_bytes)));
        }

        src_strings.extend_from_slice(&try!(self.get_kernel_includes_eof()));

        Ok(src_strings)
    }
}




/// A program from which kernels can be created from.
///
/// To use with multiple devices, create manually with `::from_parts()`.
///
/// # Destruction
///
/// Handled automatically. Feel free to store, clone, and share among threads
/// as you please.
///
#[derive(Clone, Debug)]
pub struct Program {
    obj_core: ProgramCore,
    devices: Vec<Device>,
}

impl Program {
    /// Returns a new `ProgramBuilder`.
    pub fn builder() -> ProgramBuilder {
        ProgramBuilder::new()
    }

    // /// Returns a new program.
    // pub fn new(program_builder: ProgramBuilder, context: &Context, device_idxs: Vec<usize>
    //         ) -> OclResult<Program> 
    // {
    //     let device_ids = context.resolve_wrapping_device_idxs(&device_idxs);

    //     Program::new(
    //         try!(program_builder.get_src_strings().map_err(|e| e.to_string())), 
    //         try!(program_builder.get_compiler_options().map_err(|e| e.to_string())), 
    //         context, 
    //         &device_ids)
    // }

    /// Returns a new program built from pre-created build components and device
    /// list.
    // [SOMEDAY TODO]: Keep track of line number range for each kernel string and print 
    // out during build failure.
    pub fn new(src_strings: Vec<CString>, cmplr_opts: CString, context_obj_core: &ContextCore,
                device_ids: &[Device]) -> OclResult<Program>
    {
        let obj_core = try!(core::create_build_program(context_obj_core, &src_strings, &cmplr_opts, 
             device_ids).map_err(|e| e.to_string()));

        Ok(Program {
            obj_core: obj_core,
            devices: Vec::from(device_ids),
        })
    }

    /// Returns the associated OpenCL program object.
    pub fn core_as_ref(&self) -> &ProgramCore {
        &self.obj_core
    }

    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    /// Returns info about this program.
    pub fn info(&self, info_kind: ProgramInfo) -> ProgramInfoResult {
        // match core::get_program_info(&self.obj_core, info_kind) {
        //     Ok(res) => res,
        //     Err(err) => ProgramInfoResult::Error(Box::new(err)),
        // }        
        core::get_program_info(&self.obj_core, info_kind)
    }

    /// Returns info about this program's build.
    ///
    /// TODO: Check that device is valid.
    pub fn build_info(&self, device: Device, info_kind: ProgramBuildInfo) -> ProgramBuildInfoResult {
        // match core::get_program_build_info(&self.obj_core, &device, info_kind) {
        //     Ok(res) => res,
        //     Err(err) => ProgramBuildInfoResult::Error(Box::new(err)),
        // }        
        core::get_program_build_info(&self.obj_core, &device, info_kind)
    }

    fn fmt_info(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Program")
            .field("ReferenceCount", &self.info(ProgramInfo::ReferenceCount))
            .field("Context", &self.info(ProgramInfo::Context))
            .field("NumDevices", &self.info(ProgramInfo::NumDevices))
            .field("Devices", &self.info(ProgramInfo::Devices))
            .field("Source", &self.info(ProgramInfo::Source))
            .field("BinarySizes", &self.info(ProgramInfo::BinarySizes))
            .field("Binaries", &self.info(ProgramInfo::Binaries))
            .field("NumKernels", &self.info(ProgramInfo::NumKernels))
            .field("KernelNames", &self.info(ProgramInfo::KernelNames))
            .finish()
    }
}


impl std::fmt::Display for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.fmt_info(f)
    }
}

impl Deref for Program {
    type Target = ProgramCore;

    fn deref(&self) -> &ProgramCore {
        &self.obj_core
    }
}

impl DerefMut for Program {
    fn deref_mut(&mut self) -> &mut ProgramCore {
        &mut self.obj_core
    }
}
