use crate::spec::{LinkerFlavor, Lld, Target};

pub(crate) fn target() -> Target {
    let mut base = super::i686_pc_windows_msvc::target();
    base.vendor = "rust9x".into();

    base.add_pre_link_args(LinkerFlavor::Msvc(Lld::No), &[
        // "/LARGEADDRESSAWARE",
        // "/SAFESEH",
        // ↑ these are already added in the base target

        // Link to ___CxxFrameHandler (XP and earlier MSVCRT) instead of ___CxxFrameHandler3.
        // This cannot be done in the MSVC `eh_personality` handling because LLVM hardcodes SEH
        // support based on that name, sadly
        "/ALTERNATENAME:___CxxFrameHandler3=___CxxFrameHandler",
    ]);

    base.metadata = crate::spec::TargetMetadata {
        description: Some("32-bit MSVC rust9x (Windows 95/NT3.51+)".into()),
        tier: Some(4),
        host_tools: Some(false),
        std: Some(true),
    };

    base
}
