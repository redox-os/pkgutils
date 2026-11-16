#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum DataVersion {
    V0 = 0,
    Reserved(u8),
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Architecture {
    /// Architecture-independent
    Independent = 0,
    /// x86_64, base arch (x86_64-v1)
    X86_64 = 1,
    /// 32 bit x86, base arch (i586)
    X86 = 2,
    /// Aarch64, base arch (Armv8-A)
    AArch64 = 3,
    /// Riscv64, base arch (extension GC)
    RiscV64 = 4,
    /// x86_64, v3 (x86_64-v3)
    X86_64v3 = 17,
    /// Aarch64, v8.2-A arch with optional extensions (Armv8.2-A+dotprod+fp16)
    AArch64v8_2 = 19,
    Reserved(u8),
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Packaging {
    Uncompressed = 0,
    LZMA2 = 1,
    Reserved(u8),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C, packed)]
pub struct HeaderFlags(pub u32);

impl HeaderFlags {
    pub fn new(version: DataVersion, arch: Architecture, pkg: Packaging) -> Self {
        let mut bits = 0u32;
        bits |= Self::val_version(version) as u32;
        bits |= (Self::val_arch(arch) as u32) << 8;
        bits |= (Self::val_pkg(pkg) as u32) << 16;
        Self(bits)
    }

    pub fn latest(arch: Architecture, pkg: Packaging) -> Self {
        Self::new(DataVersion::V0, arch, pkg)
    }

    pub fn version(&self) -> DataVersion {
        match self.0 as u8 {
            0 => DataVersion::V0,
            v => DataVersion::Reserved(v),
        }
    }

    pub fn architecture(&self) -> Architecture {
        match (self.0 >> 8) as u8 {
            0 => Architecture::Independent,
            1 => Architecture::X86_64,
            2 => Architecture::X86,
            3 => Architecture::AArch64,
            4 => Architecture::RiscV64,
            17 => Architecture::X86_64v3,
            19 => Architecture::AArch64v8_2,
            v => Architecture::Reserved(v),
        }
    }

    pub fn packaging(&self) -> Packaging {
        match (self.0 >> 16) as u8 {
            0 => Packaging::Uncompressed,
            1 => Packaging::LZMA2,
            v => Packaging::Reserved(v),
        }
    }

    fn val_version(v: DataVersion) -> u8 {
        match v {
            DataVersion::V0 => 0,
            DataVersion::Reserved(n) => n,
        }
    }
    fn val_arch(a: Architecture) -> u8 {
        match a {
            Architecture::Independent => 0,
            Architecture::X86_64 => 1,
            Architecture::X86 => 2,
            Architecture::AArch64 => 3,
            Architecture::RiscV64 => 4,
            Architecture::X86_64v3 => 17,
            Architecture::AArch64v8_2 => 19,
            Architecture::Reserved(n) => n,
        }
    }
    fn val_pkg(p: Packaging) -> u8 {
        match p {
            Packaging::Uncompressed => 0,
            Packaging::LZMA2 => 1,
            Packaging::Reserved(n) => n,
        }
    }
}

impl From<u32> for HeaderFlags {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<HeaderFlags> for u32 {
    fn from(val: HeaderFlags) -> Self {
        val.0
    }
}

impl Architecture {
    /// Check if this package `Architecture` flags is supported on target machine.
    pub fn is_supported(self, target: Self) -> bool {
        match self {
            Architecture::Independent => true,
            Architecture::X86_64 => matches!(target, Architecture::X86_64 | Architecture::X86_64v3),
            Architecture::X86 => matches!(target, Architecture::X86),
            Architecture::AArch64 => {
                matches!(target, Architecture::AArch64 | Architecture::AArch64v8_2)
            }
            Architecture::RiscV64 => matches!(target, Architecture::RiscV64),
            Architecture::X86_64v3 => matches!(target, Architecture::X86_64v3),
            Architecture::AArch64v8_2 => matches!(target, Architecture::AArch64v8_2),
            Architecture::Reserved(_) => false,
        }
    }

    /// Get the base architecture of this flag, if this architecture flag is an extension
    pub fn base_architecture(self) -> Option<Self> {
        match self {
            Architecture::X86_64v3 => Some(Architecture::X86_64),
            Architecture::AArch64v8_2 => Some(Architecture::AArch64),
            _ => None,
        }
    }

    /// Get the next architecture of this flag, if available
    pub fn next_architecture(&self) -> Option<Architecture> {
        match self {
            Architecture::X86_64 => Some(Architecture::X86_64v3),
            Architecture::AArch64 => Some(Architecture::AArch64v8_2),
            _ => None,
        }
    }
}
