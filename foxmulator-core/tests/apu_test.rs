//! APU register, channel and mixing tests derived from Pan Docs.

#[cfg(test)]
mod apu_tests {
    use foxmulator_core::apu::{
        NR10, NR11, NR12, NR13, NR14, NR22, NR23, NR24, NR30, NR31, NR32, NR33, NR34, NR41, NR42,
        NR43, NR44, NR50, NR51, NR52, WAVE_RAM_START,
    };
    use foxmulator_core::mmu::Mmu;

    fn apu_pair() -> (Mmu, ()) {
        (Mmu::new(), ())
    }

    #[test]
    fn power_on_defaults() {
        let (mmu, _) = apu_pair();
        assert_eq!(mmu.read_byte(NR52), 0xF0, "APU on, no channel active");
        assert_eq!(mmu.read_byte(NR10), 0x80);
        assert_eq!(mmu.read_byte(NR11), 0x3F);
        assert_eq!(mmu.read_byte(NR50), 0x77);
        assert_eq!(mmu.read_byte(NR51), 0xF3);
    }

    #[test]
    fn master_registers_roundtrip() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR50, 0x12);
        assert_eq!(mmu.read_byte(NR50), 0x12);
        mmu.write_byte(NR51, 0xA5);
        assert_eq!(mmu.read_byte(NR51), 0xA5);
    }

    #[test]
    fn square_channel_trigger_sets_nr52_bit() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR12, 0xF0); // DAC on, volume 15, no envelope
        mmu.write_byte(NR13, 0x00);
        mmu.write_byte(NR14, 0x80); // trigger, freq high = 0
        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x01, "channel 1 active");
    }

    #[test]
    fn dac_off_disables_channel() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR12, 0xF0);
        mmu.write_byte(NR14, 0x80);
        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x01);

        mmu.write_byte(NR12, 0x00); // DAC off
        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x00);
    }

    #[test]
    fn length_counter_disables_channel() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR11, 0x3F); // length = 1
        mmu.write_byte(NR12, 0xF0); // DAC on
        mmu.write_byte(NR14, 0xC0); // length enable + trigger

        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x01);
        // Frame sequencer step 0 ticks the length counter: 8192 T-cycles.
        mmu.tick(8192);
        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x00, "length expired");
    }

    #[test]
    fn sweep_overflow_disables_channel() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR10, 0x11); // period 1, shift 1, add
        mmu.write_byte(NR12, 0xF0);
        mmu.write_byte(NR13, 0xFF);
        mmu.write_byte(NR14, 0x87); // freq high = 7 -> 2047, trigger
        assert_eq!(
            mmu.read_byte(NR52) & 0x01,
            0x00,
            "initial sweep overflow must disable the channel"
        );
    }

    #[test]
    fn wave_channel_plays_wave_ram() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR30, 0x80); // DAC on
        mmu.write_byte(NR31, 0x00); // length 256
        mmu.write_byte(NR32, 0x20); // 100% volume
        mmu.write_byte(NR33, 0x00);
        mmu.write_byte(NR34, 0x80); // trigger

        // Full-scale square wave.
        for i in 0..16 {
            mmu.write_byte(WAVE_RAM_START + i, 0xFF);
        }

        assert_eq!(mmu.read_byte(NR52) & 0x04, 0x04);
        mmu.tick(2048);
        let samples = mmu.apu.take_samples();
        assert!(!samples.is_empty(), "samples are generated");
        assert!(samples.iter().any(|s| s.abs() > 0.001), "wave is audible");
    }

    #[test]
    fn noise_channel_runs_lfsr() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR42, 0xF0); // DAC on, volume 15
        mmu.write_byte(NR43, 0x00); // divisor 0, shift 0
        mmu.write_byte(NR44, 0x80); // trigger

        assert_eq!(mmu.read_byte(NR52) & 0x08, 0x08);
        mmu.tick(10_000);
        let samples = mmu.apu.take_samples();
        assert!(
            samples.iter().any(|s| s.abs() > 0.001),
            "noise channel produces audio"
        );
    }

    #[test]
    fn samples_are_bounded_stereo() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR12, 0xF0);
        mmu.write_byte(NR13, 0x00);
        mmu.write_byte(NR14, 0x80);
        mmu.tick(50_000);

        let samples = mmu.apu.take_samples();
        assert!(!samples.is_empty());
        assert_eq!(samples.len() % 2, 0, "interleaved stereo");
        assert!(samples.iter().all(|s| (-1.0..=1.0).contains(s)));
    }

    #[test]
    fn cgb_double_speed_keeps_apu_on_the_system_clock() {
        let configure_ch1 = |mmu: &mut Mmu| {
            mmu.cgb = true;
            mmu.apu.set_cgb(true);
            mmu.write_byte(NR50, 0x77);
            mmu.write_byte(NR51, 0x11);
            mmu.write_byte(NR11, 0x80);
            mmu.write_byte(NR12, 0xF0);
            mmu.write_byte(NR13, 0x40);
            mmu.write_byte(NR14, 0x80);
        };

        let (mut normal, _) = apu_pair();
        configure_ch1(&mut normal);
        normal.tick(16_384);
        let normal_samples = normal.apu.take_samples();

        let (mut double_speed, _) = apu_pair();
        configure_ch1(&mut double_speed);
        double_speed.double_speed = true;
        // Twice as many CPU clocks must advance the APU by the same number of
        // system clocks, preserving both PCM cadence and channel phase.
        double_speed.tick(32_768);
        let double_speed_samples = double_speed.apu.take_samples();

        assert_eq!(double_speed.peripheral_ticks, 16_384);
        assert_eq!(double_speed_samples, normal_samples);
    }

    #[test]
    fn cgb_double_speed_does_not_double_frame_sequencer_events() {
        let configure = |mmu: &mut Mmu| {
            mmu.cgb = true;
            mmu.apu.set_cgb(true);
            mmu.write_byte(NR11, 0x3E); // length counter = 2
            mmu.write_byte(NR12, 0xF0);
            mmu.write_byte(NR14, 0xC0);
        };

        let (mut normal, _) = apu_pair();
        configure(&mut normal);
        normal.tick(8_192);

        let (mut double_speed, _) = apu_pair();
        configure(&mut double_speed);
        double_speed.double_speed = true;
        double_speed.tick(16_384); // same 8192 peripheral cycles

        assert_eq!(normal.read_byte(NR52) & 0x01, 0x01);
        assert_eq!(double_speed.read_byte(NR52) & 0x01, 0x01);
    }

    #[test]
    fn nr51_routes_a_channel_to_the_selected_stereo_side() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR50, 0x77);
        mmu.write_byte(NR51, 0x01); // CH1 right only
        mmu.write_byte(NR12, 0xF0);
        mmu.write_byte(NR13, 0x00);
        mmu.write_byte(NR14, 0x80);
        mmu.tick(20_000);
        let samples = mmu.apu.take_samples();
        let (pairs, []) = samples.as_chunks::<2>() else {
            panic!("audio must be interleaved stereo");
        };
        let (left, right) = pairs.iter().fold((0.0f32, 0.0f32), |(l, r), pair| {
            (l + pair[0].abs(), r + pair[1].abs())
        });
        assert!(
            left < 0.001,
            "CH1 must be absent from left when NR51 bit 4 is clear"
        );
        assert!(right > 0.1, "CH1 must reach right when NR51 bit 0 is set");
    }

    #[test]
    fn nr50_scales_the_master_stereo_volume() {
        let render = |nr50| {
            let (mut mmu, _) = apu_pair();
            mmu.write_byte(NR50, nr50);
            mmu.write_byte(NR51, 0x11);
            mmu.write_byte(NR12, 0xF0);
            mmu.write_byte(NR14, 0x80);
            mmu.tick(20_000);
            mmu.apu
                .take_samples()
                .iter()
                .map(|sample| sample.abs())
                .sum::<f32>()
        };
        // Hardware encodes output level as SOx volume + 1: zero is minimum,
        // not a mute. NR51, rather than NR50, disconnects a channel.
        assert!(render(0x77) > render(0x00) * 3.0);
    }

    #[test]
    fn power_off_clears_registers_but_keeps_wave_ram() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(WAVE_RAM_START, 0xAB);
        mmu.write_byte(NR50, 0x12);
        mmu.write_byte(NR52, 0x00); // power off

        assert_eq!(mmu.read_byte(NR52) & 0x80, 0);
        assert_eq!(mmu.read_byte(NR50), 0x00);
        assert_eq!(mmu.read_byte(WAVE_RAM_START), 0xAB);
    }

    #[test]
    fn writes_ignored_while_powered_off_except_nr52_and_wave() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR52, 0x00);
        mmu.write_byte(NR50, 0x12);
        assert_eq!(mmu.read_byte(NR50), 0x00, "NR50 write ignored while off");

        mmu.write_byte(WAVE_RAM_START + 1, 0x5A);
        assert_eq!(mmu.read_byte(WAVE_RAM_START + 1), 0x5A);

        mmu.write_byte(NR52, 0x80); // power back on
        assert_eq!(mmu.read_byte(NR52) & 0x80, 0x80);
    }

    #[test]
    fn timer_phase_is_preserved_across_apu_power_cycle() {
        let (mut fresh, _) = apu_pair();
        let (mut cycled, _) = apu_pair();

        // Put the cycled APU at a non-zero point in the 8192-T-cycle frame
        // sequencer, then power it off and on before configuring CH1.
        cycled.tick(7_000);
        cycled.write_byte(NR52, 0x00);
        cycled.write_byte(NR52, 0x80);

        for mmu in [&mut fresh, &mut cycled] {
            mmu.write_byte(NR11, 0x3F); // length counter = 1
            mmu.write_byte(NR12, 0xF0); // DAC enabled
            mmu.write_byte(NR14, 0xC0); // trigger with length enabled
        }

        fresh.tick(8_192);
        cycled.tick(1_192);
        assert_eq!(fresh.read_byte(NR52) & 0x01, 0x00);
        assert_eq!(cycled.read_byte(NR52) & 0x01, 0x01);
    }

    #[test]
    fn div_reset_in_second_frame_half_clocks_apu_immediately() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR11, 0x3F); // length counter = 1
        mmu.write_byte(NR12, 0xF0);
        mmu.write_byte(NR14, 0xC0);
        mmu.tick(4_096); // second half of the 8192-cycle frame period
        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x01);
        mmu.write_byte(foxmulator_core::timer_mod::DIV, 0);
        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x00);
    }

    #[test]
    fn div_reset_restarts_the_following_apu_frame_period() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR11, 0x3E); // length counter = 2
        mmu.write_byte(NR12, 0xF0);
        mmu.write_byte(NR14, 0xC0);

        mmu.tick(6_000);
        mmu.write_byte(foxmulator_core::timer_mod::DIV, 0);
        // DIV's falling edge consumed one length clock, leaving one.
        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x01);

        // The first post-reset event is sequence step 1 and does not clock
        // length. Without resetting the shared phase, the *second* event
        // (step 2) would arrive after only 10384 cycles total.
        mmu.tick(10_384);
        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x01);
        mmu.tick(6_000);
        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x00);
    }

    #[test]
    fn channel_2_registers_are_independent() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR22, 0xF0);
        mmu.write_byte(NR23, 0x00);
        mmu.write_byte(NR24, 0x80);
        assert_eq!(mmu.read_byte(NR52) & 0x02, 0x02);
        assert_eq!(mmu.read_byte(NR52) & 0x01, 0x00);
    }

    #[test]
    fn noise_and_wave_register_masks() {
        let (mut mmu, _) = apu_pair();
        mmu.write_byte(NR41, 0x00);
        assert_eq!(mmu.read_byte(NR41), 0xFF);
        mmu.write_byte(NR30, 0x80);
        assert_eq!(mmu.read_byte(NR30), 0xFF);
        mmu.write_byte(NR32, 0x20);
        assert_eq!(mmu.read_byte(NR32), 0xBF);
    }
}
