use crate::spec::Target;

pub(crate) fn target() -> Target {
    let mut base = super::i686_rust9x_windows_msvc::target();
    base.cpu = "pentium".into();
    base.llvm_target = "i586-pc-windows-msvc".into();
    // go back to x87 FPU ABI spec
    base.rustc_abi = None;
    base
}
