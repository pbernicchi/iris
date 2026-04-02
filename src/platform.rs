// Platform specific implementations

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub mod x86 {
    use std::arch::asm;

    /// Get high-resolution host timer (RDTSC)
    #[inline]
    pub fn get_host_ticks() -> u64 {
        let rax: u64;
        let rdx: u64;
        unsafe {
            asm!(
                "rdtsc",
                out("rax") rax,
                out("rdx") rdx,
                options(nomem, nostack)
            );
        }
        (rdx << 32) | rax
    }

    /// Get host timer frequency in Hz
    pub fn get_host_tick_frequency() -> u64 {
        static mut FREQ: u64 = 0;
        static INIT: std::sync::Once = std::sync::Once::new();
        
        unsafe {
            INIT.call_once(|| {
                // Calibrate TSC against std::time::Instant
                let start = std::time::Instant::now();
                let start_tsc = get_host_ticks();
                
                // Spin for 10ms to get a decent sample
                while start.elapsed() < std::time::Duration::from_millis(10) {
                    std::hint::spin_loop();
                }
                
                let end_tsc = get_host_ticks();
                let elapsed = start.elapsed();
                
                // Calculate frequency
                let nanos = elapsed.as_nanos() as u64;
                if nanos > 0 {
                    FREQ = (end_tsc.wrapping_sub(start_tsc)) * 1_000_000_000 / nanos;
                } else {
                    FREQ = 1_000_000_000; // Fallback
                }
            });
            FREQ
        }
    }

    /// Set FPU rounding mode to match MIPS FCSR
    /// MIPS RM (bits 1:0): 0=RN, 1=RZ, 2=RP, 3=RM
    /// x86 MXCSR (bits 14:13): 00=RN, 01=RM, 10=RP, 11=RZ
    pub fn set_fpu_mode(mips_rm: u8) {
        let x86_rm = match mips_rm & 0x3 {
            0 => 0b00, // Nearest
            1 => 0b11, // Zero (Truncate)
            2 => 0b10, // +Inf (Up)
            3 => 0b01, // -Inf (Down)
            _ => unreachable!(),
        };

        unsafe {
            let mut mxcsr: u32 = 0;
            // Read MXCSR
            asm!("stmxcsr [{}]", in(reg) &mut mxcsr, options(nostack));
            
            // Clear rounding bits (14:13)
            mxcsr &= !(0x3 << 13);
            // Set new rounding bits
            mxcsr |= x86_rm << 13;
            
            // Write MXCSR
            asm!("ldmxcsr [{}]", in(reg) &mxcsr, options(nostack));
        }
    }

    /// Get FPU status flags translated to MIPS FCSR format (bits 6:2)
    pub fn get_fpu_status() -> u32 {
        let mut mxcsr: u32 = 0;
        unsafe {
            asm!("stmxcsr [{}]", in(reg) &mut mxcsr, options(nostack));
        }
        
        let mut mips_flags = 0;
        if (mxcsr & 0x01) != 0 { mips_flags |= 1 << 6; } // Invalid -> V
        if (mxcsr & 0x04) != 0 { mips_flags |= 1 << 5; } // DivZero -> Z
        if (mxcsr & 0x08) != 0 { mips_flags |= 1 << 4; } // Overflow -> O
        if (mxcsr & 0x10) != 0 { mips_flags |= 1 << 3; } // Underflow -> U
        if (mxcsr & 0x20) != 0 { mips_flags |= 1 << 2; } // Inexact -> I
        
        mips_flags
    }

    /// Clear FPU status flags
    pub fn clear_fpu_status() {
        unsafe {
            let mut mxcsr: u32 = 0;
            asm!("stmxcsr [{}]", in(reg) &mut mxcsr, options(nostack));
            mxcsr &= !0x3F; // Clear sticky flags
            asm!("ldmxcsr [{}]", in(reg) &mxcsr, options(nostack));
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub mod aarch64 {
    use std::arch::asm;

    /// Get high-resolution host timer (CNTVCT_EL0)
    #[inline]
    pub fn get_host_ticks() -> u64 {
        let ticks: u64;
        unsafe {
            // Use virtual counter frequency
            asm!("mrs {}, cntvct_el0", out(reg) ticks, options(nomem, nostack));
        }
        ticks
    }

    /// Get host timer frequency in Hz
    #[inline]
    pub fn get_host_tick_frequency() -> u64 {
        let freq: u64;
        unsafe {
            asm!("mrs {}, cntfrq_el0", out(reg) freq, options(nomem, nostack));
        }
        freq
    }

    /// Set FPU rounding mode to match MIPS FCSR
    /// MIPS RM (bits 1:0): 0=RN, 1=RZ, 2=RP, 3=RM
    /// ARM64 FPCR (bits 23:22): 00=RN, 01=RP, 10=RM, 11=RZ
    pub fn set_fpu_mode(mips_rm: u8) {
        let arm_rm = match mips_rm & 0x3 {
            0 => 0b00, // Nearest
            1 => 0b11, // Zero
            2 => 0b01, // +Inf
            3 => 0b10, // -Inf
            _ => unreachable!(),
        };

        unsafe {
            let mut fpcr: u64;
            asm!("mrs {}, fpcr", out(reg) fpcr, options(nomem, nostack));
            
            // Clear RMode bits (23:22)
            fpcr &= !(0x3 << 22);
            // Set new RMode bits
            fpcr |= (arm_rm as u64) << 22;
            
            asm!("msr fpcr, {}", in(reg) fpcr, options(nomem, nostack));
        }
    }

    /// Get FPU status flags translated to MIPS FCSR format (bits 6:2)
    pub fn get_fpu_status() -> u32 {
        let fpsr: u64;
        unsafe {
            asm!("mrs {}, fpsr", out(reg) fpsr, options(nomem, nostack));
        }
        
        let mut mips_flags = 0;
        if (fpsr & 0x01) != 0 { mips_flags |= 1 << 6; } // Invalid -> V
        if (fpsr & 0x02) != 0 { mips_flags |= 1 << 5; } // DivZero -> Z
        if (fpsr & 0x04) != 0 { mips_flags |= 1 << 4; } // Overflow -> O
        if (fpsr & 0x08) != 0 { mips_flags |= 1 << 3; } // Underflow -> U
        if (fpsr & 0x10) != 0 { mips_flags |= 1 << 2; } // Inexact -> I
        
        mips_flags
    }

    /// Clear FPU status flags
    pub fn clear_fpu_status() {
        unsafe {
            let zero: u64 = 0;
            asm!("msr fpsr, {}", in(reg) zero, options(nomem, nostack));
        }
    }
}

// Fallback
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
pub mod generic {
    pub fn get_host_ticks() -> u64 {
        // Low resolution fallback
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    pub fn get_host_tick_frequency() -> u64 {
        1_000_000_000
    }

    pub fn set_fpu_mode(_mips_rm: u8) {
        // Not implemented
    }

    pub fn get_fpu_status() -> u32 {
        0
    }

    pub fn clear_fpu_status() {
        // Not implemented
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use x86::*;

#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
pub use generic::*;