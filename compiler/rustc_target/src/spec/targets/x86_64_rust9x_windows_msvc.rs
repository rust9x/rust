use crate::spec::{LinkerFlavor, Lld, Target};

pub(crate) fn target() -> Target {
    let mut base = super::x86_64_pc_windows_msvc::target();
    // these aren't available on all x86_64 CPUs
    // base.features = "+cx16,+sse3,+sahf".into();
    base.plt_by_default = false;
    base.max_atomic_width = Some(64);
    base.vendor = "rust9x".into();

    base.add_pre_link_args(LinkerFlavor::Msvc(Lld::No), &[
        // Link to ___CxxFrameHandler (XP and earlier MSVCRT) instead of ___CxxFrameHandler3.
        // This cannot be done in the MSVC `eh_personality` handling because LLVM hardcodes SEH
        // support based on that name, sadly
        "/ALTERNATENAME:___CxxFrameHandler3=___CxxFrameHandler",
    ]);

    base.metadata = crate::spec::TargetMetadata {
        description: Some("64-bit MSVC rust9x (Windows XP 64bit+)".into()),
        tier: Some(4),
        host_tools: Some(false),
        std: Some(true),
    };

    base
}
