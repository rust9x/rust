use crate::spec::{Target, cvs};

pub(crate) fn target() -> Target {
    let mut base = super::x86_64_pc_windows_gnu::target();
    base.families = cvs!["windows", "rust9x"];
    base.has_thread_local = false;

    base.metadata = crate::spec::TargetMetadata {
        description: Some("32-bit GNU rust9x (Windows 98/NT4+)".into()),
        tier: Some(4),
        host_tools: Some(false),
        std: Some(true),
    };

    base
}
