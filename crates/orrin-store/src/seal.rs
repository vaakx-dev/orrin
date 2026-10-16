use std::fs::OpenOptions;
use std::io;
use std::path::Path;

pub fn is_sealed(path: &Path) -> bool {
    OpenOptions::new().write(true).open(path).is_err_and(|err| err.kind() == io::ErrorKind::PermissionDenied)
}

#[cfg(windows)]
pub fn seal(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_FILE_OBJECT, SetNamedSecurityInfoW,
    };
    use windows_sys::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, PSECURITY_DESCRIPTOR,
        UNPROTECTED_DACL_SECURITY_INFORMATION,
    };

    let wide = |text: &std::ffi::OsStr| text.encode_wide().chain([0]).collect::<Vec<u16>>();
    let sddl = wide("D:(D;;0x6;;;WD)".as_ref());
    let name = wide(path.as_os_str());
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let mut dacl: *mut ACL = null_mut();
    let (mut present, mut defaulted) = (0, 0);
    unsafe {
        let made = ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        );
        if made == 0 {
            return Err(io::Error::last_os_error());
        }
        GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted);
        let info = DACL_SECURITY_INFORMATION | UNPROTECTED_DACL_SECURITY_INFORMATION;
        let status =
            SetNamedSecurityInfoW(name.as_ptr(), SE_FILE_OBJECT, info, null_mut(), null_mut(), dacl, null_mut());
        LocalFree(descriptor);
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
    }
    Ok(())
}

#[cfg(unix)]
pub fn seal(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)?.permissions().mode() & !0o222;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(unix)]
pub fn unseal(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)?.permissions().mode() | 0o200;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
pub fn unseal(_path: &Path) -> io::Result<()> {
    Ok(())
}
