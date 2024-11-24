use crate::sys::c;

#[cfg(not(any(target_vendor = "win7", target_vendor = "rust9x")))]
#[inline]
pub fn fill_bytes(bytes: &mut [u8]) {
    let ret = unsafe { c::ProcessPrng(bytes.as_mut_ptr(), bytes.len()) };
    // ProcessPrng is documented as always returning `TRUE`.
    // https://learn.microsoft.com/en-us/windows/win32/seccng/processprng#return-value
    debug_assert_eq!(ret, c::TRUE);
}

#[cfg(target_vendor = "win7")]
pub fn fill_bytes(mut bytes: &mut [u8]) {
    while !bytes.is_empty() {
        let len = bytes.len().try_into().unwrap_or(u32::MAX);
        let ret = unsafe { c::RtlGenRandom(bytes.as_mut_ptr().cast(), len) };
        assert_ne!(ret, 0, "failed to generate random data");
        bytes = &mut bytes[len as usize..];
    }
}

#[cfg(target_vendor = "rust9x")]
mod rust9x {
    use super::*;
    use crate::sys::sync::OnceBox;

    pub fn fill_bytes(mut bytes: &mut [u8]) {
        if let Some(f) = c::RtlGenRandom::available() {
            while !bytes.is_empty() {
                let len = bytes.len().try_into().unwrap_or(u32::MAX);
                let ret = unsafe { f(bytes.as_mut_ptr().cast(), len) };
                assert_ne!(ret, 0, "failed to generate random data");
                bytes = &mut bytes[len as usize..];
            }
        } else if let Some(f) = c::CryptGenRandom::available() {
            let ctx = CRYPT_CONTEXT.get_or_init(init_crypt_context);
            while !bytes.is_empty() {
                let len = bytes.len().try_into().unwrap_or(u32::MAX);
                let ret = unsafe { f(ctx.0, len, bytes.as_mut_ptr().cast()) };
                assert_ne!(ret, 0, "failed to generate random data");
                bytes = &mut bytes[len as usize..];
            }
        } else {
            // well, we tried, fall back to a non-cryptographically-secure PRNG
            // for NT <4.0 and 95 without IE3.02 or higher.

            // seed with stack address and tick count
            let mut state: [u32; 2] = [unsafe { c::GetTickCount() }, 0];
            state[1] = (&raw const state) as u32;

            let mut chunks = bytes.chunks_exact_mut(4);
            for chunk in &mut chunks {
                let [a, b, c, d] = xoroshiro64_star_star(&mut state).to_ne_bytes();
                chunk[0] = a;
                chunk[1] = b;
                chunk[2] = c;
                chunk[3] = d;
            }

            let remainder = chunks.into_remainder();
            if remainder.is_empty() {
                return;
            }

            for (rem, val) in
                remainder.iter_mut().zip(xoroshiro64_star_star(&mut state).to_ne_bytes())
            {
                *rem = val;
            }
        }
    }

    static CRYPT_CONTEXT: OnceBox<HCryptProvider> = OnceBox::new();

    struct HCryptProvider(usize);
    impl Drop for HCryptProvider {
        fn drop(&mut self) {
            unsafe {
                c::CryptReleaseContext(self.0, 0);
            }
        }
    }

    fn init_crypt_context() -> Box<HCryptProvider> {
        let mut crypt_context = 0;
        unsafe {
            let ret = c::CryptAcquireContextA(
                &mut crypt_context,
                core::ptr::null(),
                core::ptr::null(),
                c::PROV_RSA_FULL,
                c::CRYPT_VERIFYCONTEXT,
            );
            assert_ne!(ret, c::FALSE, "failed to acquire crypt context: {:#X}", c::GetLastError());
        };
        Box::new(HCryptProvider(crypt_context))
    }

    // xoroshiro64**
    // 2018 by David Blackman and Sebastiano Vigna (vigna@acm.org)
    // https://prng.di.unimi.it/xoroshiro64starstar.c
    fn xoroshiro64_star_star(state: &mut [u32; 2]) -> u32 {
        let result = state[0].wrapping_mul(0x9E3779BB).rotate_left(5).wrapping_mul(5);
        state[1] ^= state[0];
        state[0] = state[0].rotate_left(26) ^ state[1] ^ (state[1] << 9);
        state[1] = state[1].rotate_left(13);

        result
    }
}

#[cfg(target_vendor = "rust9x")]
pub use rust9x::fill_bytes;
