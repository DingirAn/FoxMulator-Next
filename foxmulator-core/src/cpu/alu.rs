//! ALU: operaciones aritméticas y lógicas del SM83
//! Todas las funciones operan sobre u16 para prevenir desbordamientos durante el cálculo.

/// Resultado de una operación aritmética 8-bit
#[derive(Debug, Clone, Copy)]
pub struct AluResult8 {
    pub result: u8,
    pub carry: bool,
    pub half_carry: bool,
}

/// Resultado de una operación aritmética 16-bit
#[derive(Debug, Clone, Copy)]
pub struct AluResult16 {
    pub result: u16,
    pub carry: bool,
    pub half_carry: bool,
}

/// Operación ADD sin carry — usada por ADD A,r y ADD HL,rr
#[inline]
pub fn alu_add8(a: u16, b: u16) -> AluResult8 {
    let result = a + b;
    let half_carry = ((a & 0x0F) + (b & 0x0F)) > 0x0F;
    let carry = result > 0xFF;
    AluResult8 {
        result: (result & 0xFF) as u8,
        carry,
        half_carry,
    }
}

/// Operación SUB sin borrow — usada por SUB A,r
#[inline]
pub fn alu_sub8(a: u16, b: u16) -> AluResult8 {
    let result = a.wrapping_sub(b);
    let half_carry = (a & 0x0F) < (b & 0x0F);
    let carry = a < b;
    AluResult8 {
        result: result as u8,
        carry,
        half_carry,
    }
}

/// Operación ADC — sum con carry de flags
#[inline]
pub fn alu_adc8(a: u16, b: u16, c_in: bool) -> AluResult8 {
    let c_val = if c_in { 1 } else { 0 };
    let result = a + b + c_val as u16;
    let half_carry = ((a & 0x0F) + (b & 0x0F) + c_val as u16) > 0x0F;
    let carry = result > 0xFF;
    AluResult8 {
        result: (result & 0xFF) as u8,
        carry,
        half_carry,
    }
}

/// Operación SBC — restar con borrow de flags
#[inline]
pub fn alu_sbc8(a: u16, b: u16, c_in: bool) -> AluResult8 {
    let c_val = if c_in { 1 } else { 0 };
    let subtrahend = b + c_val as u16;
    let result = a.wrapping_sub(subtrahend);
    // Half-borrow is computed from the low nibble of both operands *plus*
    // the incoming borrow; masking `subtrahend & 0x0F` after the addition
    // loses the carry-in (e.g. 0x7F - 0x0F - 1 must set H).
    let half_carry = (a & 0x0F) < ((b & 0x0F) + c_val as u16);
    let carry = a < subtrahend;
    AluResult8 {
        result: result as u8,
        carry,
        half_carry,
    }
}

/// Operación AND — lógica bit a bit con flags
#[inline]
pub fn alu_and8(a: u16, b: u16) -> AluResult8 {
    let result = (a & b) as u8;
    AluResult8 {
        result,
        carry: false,
        half_carry: true, // H set for AND (always), C cleared
    }
}

/// Operación OR — lógica bit a bit con flags
#[inline]
pub fn alu_or8(a: u16, b: u16) -> AluResult8 {
    let result = (a | b) as u8;
    AluResult8 {
        result,
        carry: false,
        half_carry: false, // H cleared for OR
    }
}

/// Operación XOR — lógica bit a bit con flags
#[inline]
pub fn alu_xor8(a: u16, b: u16) -> AluResult8 {
    let result = (a ^ b) as u8;
    AluResult8 {
        result,
        carry: false,
        half_carry: false, // H cleared for XOR
    }
}

/// Operación CP — compara (equivalente a SUB pero no almacena resultado)
#[inline]
pub fn alu_cp8(a: u16, b: u16) -> AluResult8 {
    alu_sub8(a, b)
}

/// Operación ADD 16-bit — usada por ADD HL,rr y ADC/SBC de 16 bits indirectos
#[inline]
pub fn alu_add16(a: u16, b: u16) -> AluResult16 {
    let result = a.wrapping_add(b);
    let half_carry = ((a & 0x0FFF) + (b & 0x0FFF)) > 0x0FFF;
    let carry = result < a; // wrapping indicates overflow for unsigned
    AluResult16 {
        result,
        half_carry,
        carry,
    }
}

/// Operación INC — incrementa en 1 con flags correctos para DMG
#[inline]
pub fn alu_inc8(val: u8) -> u8 {
    let prev = val as u16;
    let new_val = (prev + 1) & 0xFF;
    // N = 0, H = ((val & 0xF) + 1) > 0xF, result checked by caller for Z
    if (val & 0x0F) == 0x0F { /* half carry set */ }
    new_val as u8
}

/// Operación DEC — decrementa en 1 con flags correctos para DMG
#[inline]
pub fn alu_dec8(val: u8) -> u8 {
    let prev = val as u16;
    ((prev - 1) & 0xFF) as u8
}
