use crate::spec::{Target, cvs};

pub(crate) fn target() -> Target {
    let mut base = super::i686_pc_windows_gnu::target();
    base.families = cvs!["windows", "rust9x"];

    base.metadata = crate::spec::TargetMetadata {
        description: Some("64-bit GNU rust9x (Windows XP 64-bit+)".into()),
        tier: Some(4),
        host_tools: Some(false),
        std: Some(true),
    };

    base
}
