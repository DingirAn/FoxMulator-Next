//! Registros A-F, BC, DE, HL y SP del SM83

/// Registros de 16 bits del CPU SM83 (BC, DE, HL, SP, PC)
/// + Registro A (Acumulador) y F (Flags, tratado por separado).
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct Registers {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
}

impl Registers {
    #[inline]
    pub fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }
    #[inline]
    pub fn de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }
    #[inline]
    pub fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | (self.l as u16)
    }

    #[inline]
    pub fn sp(&self) -> u16 {
        self.sp
    }

    #[inline]
    pub fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = val as u8;
    }
    #[inline]
    pub fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = val as u8;
    }
    #[inline]
    pub fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = val as u8;
    }

    #[inline]
    pub fn set_sp(&mut self, val: u16) {
        self.sp = val;
    }

    /// Acceso indirecto vía HL como puntero de memoria (ld [HL], A / ld A, [HL])
    #[inline]
    pub fn hl_addr(&self) -> u16 {
        self.hl()
    }
}
