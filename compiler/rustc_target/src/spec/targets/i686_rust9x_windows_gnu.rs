use crate::spec::{Cc, LinkerFlavor, Lld, Target, TargetOptions, add_link_args, cvs};

pub(crate) fn target() -> Target {
    let mut base = super::i686_pc_windows_gnu::target();
    base.families = cvs!["windows", "rust9x"];

    let mingw_libs = &[
        "-lunicows", // Required for Unicode on Windows 9x
        // The rest below are copied from the windows_gnu target
        "-lmsvcrt",
        "-lmingwex",
        "-lmingw32",
        "-lgcc",
        "-lmsvcrt",
        "-lmingwex",
        "-luser32",
        "-lkernel32",
    ];
    let mut late_link_args =
        TargetOptions::link_args(LinkerFlavor::Gnu(Cc::No, Lld::No), mingw_libs);
    add_link_args(&mut late_link_args, LinkerFlavor::Gnu(Cc::Yes, Lld::No), mingw_libs);
    base.late_link_args = late_link_args;

    base.metadata = crate::spec::TargetMetadata {
        description: Some("64-bit GNU rust9x (Windows XP 64-bit+)".into()),
        tier: Some(4),
        host_tools: Some(false),
        std: Some(true),
    };

    // alignment characteristics on Win7 and earlier are bad (see win7 target)
    base.options.has_thread_local = false;

    base
}
