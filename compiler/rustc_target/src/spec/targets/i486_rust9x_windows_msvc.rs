use crate::spec::Target;

pub(crate) fn target() -> Target {
    let mut base = super::i586_rust9x_windows_msvc::target();
    base.cpu = "i486".into();
    base.llvm_target = "i486-pc-windows-msvc".into();
    // no support for cmpxchg8b on i486, so AtomicU64/AtomicI64 are not supported
    base.options.max_atomic_width = Some(32);
    base
}
