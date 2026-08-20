use std::collections::HashSet;
use std::fs;

use pkgar_core::Architecture;

pub struct CpuDetection {
    extensions: HashSet<String>,
    pub eligible: Architecture,
}

impl CpuDetection {
    pub fn probe() -> Self {
        let mut extensions = HashSet::new();
        let mut eligible;

        // TODO: the cpuinfo for linux is not accurate, lists below sticks to redox os kernel
        let cpuinfo = fs::read_to_string("/scheme/sys/cpu")
            .or_else(|_| fs::read_to_string("/proc/cpuinfo"))
            .unwrap_or_default();

        // Extract features/flags from the first core
        for line in cpuinfo.lines() {
            let line = line.to_lowercase();
            if line.starts_with("features") || line.starts_with("flags") {
                if let Some((_, flags)) = line.split_once(':') {
                    for flag in flags.split_whitespace() {
                        extensions.insert(flag.to_string());
                    }
                    break;
                }
            }
        }

        #[cfg(target_arch = "x86_64")]
        {
            eligible = Architecture::X86_64;
            if Self::has_all_requirements(&extensions, Architecture::X86_64v3) {
                eligible = Architecture::X86_64v3;
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            eligible = Architecture::AArch64;
            if Self::has_all_requirements(&extensions, Architecture::AArch64v8_2) {
                eligible = Architecture::AArch64v8_2;
            }
        }

        #[cfg(target_arch = "x86")]
        {
            eligible = Architecture::X86;
        }

        #[cfg(target_arch = "riscv64")]
        {
            eligible = Architecture::RiscV64;
        }

        Self {
            extensions,
            eligible,
        }
    }

    pub fn next_eligible(&self) -> Option<Architecture> {
        // TODO: move to pkgar
        match self.eligible {
            Architecture::X86_64 => Some(Architecture::X86_64v3),
            Architecture::AArch64 => Some(Architecture::AArch64v8_2),
            _ => None,
        }
    }

    fn has_all_requirements(extensions: &HashSet<String>, arch: Architecture) -> bool {
        Self::required_for(arch)
            .iter()
            .all(|&req| extensions.contains(req))
    }

    pub fn unsatisfied_extensions(&self, base: Architecture) -> Vec<String> {
        Self::required_for(base)
            .into_iter()
            .filter(|&req| !self.extensions.contains(req))
            .map(String::from)
            .collect()
    }

    fn required_for(arch: Architecture) -> Vec<&'static str> {
        match arch {
            // https://en.wikipedia.org/wiki/X86-64#Microarchitecture_levels
            Architecture::X86_64v3 => vec![
                // x86-64-v1
                "cx8",  // CMPXCHG8B
                "cmov", // CMOV
                "mmx",  // MMX
                "fxsr", // FXSR
                "sse",  // SSE
                "sse2", // SSE2
                // x86-64-v2
                "cx16",    // CMPXCHG16B
                "lahf_lm", // LAHF/SAHF
                "popcnt",  // POPCNT
                "sse3",    // SSE3
                "ssse3",   // SSSE3
                "sse4_1",  // SSE4.1
                "sse4_2",  // SSE4.2
                // x86-64-v3
                "avx",      // AVX
                "avx2",     // AVX2
                "bmi1",     // BMI1
                "bmi2",     // BMI2
                "f16c",     // F16C
                "fma",      // FMA
                "lzcnt",    // LZCNT
                "movbe",    // MOVBE
                "xsave",    // XSAVE
                "xsaveopt", // OSXSAVE
            ],
            // https://gcc.gnu.org/onlinedocs/gcc/AArch64-Options.html#index-march
            Architecture::AArch64v8_2 => vec![
                // v8.1
                "crc", // FEAT_CRC
                "lse", // FEAT_LSE
                // v8.2
                "rdm", // FEAT_RDM
                // v8.2 optional
                "dotprod", // FEAT_DOTPROD
                           // "fp16", FEAT_FP16 (FIXME: not added in kernel)
            ],
            Architecture::X86_64 => vec![],
            Architecture::AArch64 => vec![],
            _ => vec![],
        }
    }
}
