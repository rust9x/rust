use crate::spec::Target;

pub(crate) fn target() -> Target {
    let mut base = super::i686_rust9x_windows_gnu::target();
    base.cpu = "i586".into();
    base.llvm_target = "i586-pc-windows-gnu".into();
    // go back to x87 FPU ABI spec
    base.rustc_abi = None;
    base
}
