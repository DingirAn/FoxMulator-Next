#[cfg(test)]
mod tests {
    use foxmulator_core::mmu::Mmu;

    #[test]
    fn mmu_read_write() {
        let mut mmu = Mmu::new();
        mmu.write_byte(0x1234, 0xAB);
        assert_eq!(mmu.read_byte(0x1234), 0xAB);
    }

    #[test]
    fn mmu_load_rom() {
        let mut mmu = Mmu::new();
        let data = [0x01, 0x02, 0x03];
        mmu.load_rom(0x0100, &data);
        assert_eq!(mmu.read_byte(0x0100), 0x01);
        assert_eq!(mmu.read_byte(0x0101), 0x02);
        assert_eq!(mmu.read_byte(0x0102), 0x03);
    }
}
