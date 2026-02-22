<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/rust-lang/www.rust-lang.org/master/static/images/rust-social-wide-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/rust-lang/www.rust-lang.org/master/static/images/rust-social-wide-light.svg">
    <img alt="The Rust Programming Language: A language empowering everyone to build reliable and efficient software"
         src="https://raw.githubusercontent.com/rust-lang/www.rust-lang.org/master/static/images/rust-social-wide-light.svg"
         width="50%">
  </picture>
</div>

This is the main source code repository for [Rust9x]. It contains the compiler,
standard library, and documentation.

Note that this project can not only used on Windows 10 and 11, but also on Windows 7, XP, even 
Windows 2000, 98 and 95

# Links

## Official

[Cargo]: https://github.com/rust-lang/cargo
[rustfmt]: https://github.com/rust-lang/rustfmt
[Clippy]: https://github.com/rust-lang/rust-clippy
[rust-analyzer]: https://github.com/rust-lang/rust-analyzer

## Rust9x
[Wiki]: https://github.com/rust9x/rust/wiki

# Build

If you want to build this source by yourself, you can follow [this instructions](https://github.com/rust9x/rust/wiki#installation) and set up your target.

# Install

If you think build this source is so sucks, perhaps you can follow this:

1. Download the latest release

View [here](https://github.com/zhangxuan/rust9x/releases/latest) and choose the best platform for you.

If you don't know how to choose, you can click [here](#choose-the-platform)

## Choose the platform

When you visited the release page, you might see these assets:
- `rust9x-toolchain-<version>-x86_64-windows.tar.gz`
- `rust9x-toolchain-<version>-i686-windows.tar.gz`
- `rust9x-toolchain-<version>-i586-windows.tar.gz`

For each assets has its own platform, you can choose the one that fits the target system.

### x86_64 Windows

File name: `rust9x-toolchain-<version>-x86_64-windows.tar.gz`

Support targets:
- Windows 10 and 11
- x86_64 systems (simply called "64-bit systems")

### i686 Windows

File name: `rust9x-toolchain-<version>-i686-windows.tar.gz`

Support targets:
- Windows 7 and XP
- i686 systems (simply called "32-bit systems")

### i586 Windows

File name: `rust9x-toolchain-<version>-i586-windows.tar.gz`

Support targets:
- Windows 2000, Windows 9x and older
- i586 systems (simply called "16-bit systems")

For more information, please visit [here](https://github.com/rust9x/rust/wiki#installation)

# Contributing

Thanks for your interest in contributing to this project!

If you want to know more, perhaps you can see [CONTRIBUTING.md](CONTRIBUTING.md) for more details.

# License

Rust is primarily distributed under the terms of both the MIT license and the
Apache License (Version 2.0), with portions covered by various BSD-like
licenses.

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT), and
[COPYRIGHT](COPYRIGHT) for details.

# The Rust Code of Conduct

The Code of Conduct for this repository can be found [here](https://www.rust-lang.org/conduct.html).

# Trademark

[The Rust Foundation][rust-foundation] owns and protects the Rust and Cargo
trademarks and logos (the "Rust Trademarks").

If you want to use these names or brands, please read the
[Rust language trademark policy][trademark-policy].

Third-party logos may be subject to third-party copyrights and trademarks. See
[Licenses][policies-licenses] for details.

[rust-foundation]: https://rustfoundation.org/
[trademark-policy]: https://rustfoundation.org/policy/rust-trademark-policy/
[policies-licenses]: https://www.rust-lang.org/policies/licenses