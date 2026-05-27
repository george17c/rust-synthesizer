#![no_std]
#![no_main]

mod audio;
mod video;
mod input;
mod synth;

use audio::audio_task;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use crate::{input::page_task, synth::effects::DisplayPage};

use video::DisplayManager;
use input::{input_task, key_task, btn_play_task, btn_rec_task};
use synth::{make_wtable, wave_sine, wave_triangle, wave_saw};
use synth::effects::SynthState;

use embassy_stm32::{exti::ExtiInput, gpio::Input, timer::qei::{self, Qei}};
use embassy_stm32::gpio::Pull;
use core::sync::atomic::{AtomicI16};
use embassy_executor::Spawner;

use {defmt_rtt as _, panic_probe as _};
use embassy_stm32::{
    adc::AdcChannel, bind_interrupts, dma, gpio::{Level, Output, Speed},
    interrupt::typelevel::{EXTI0, EXTI1, EXTI2, EXTI3, EXTI5, EXTI6, EXTI8, EXTI10, EXTI11, EXTI12, EXTI13, EXTI14, EXTI15},
    peripherals,
    sai::{Config, DataSize, MasterClockDivider, Mode, Protocol, Sai, TxRx, split_subblocks, word},
    spi::{Config as SpiConfig, Spi}, time::Hertz,
};
use embassy_stm32::adc::{Adc};
use embassy_time::Delay;
use display_interface_spi::SPIInterface;
use embedded_hal_bus::{
    spi::{ExclusiveDevice},
};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use core::cell::RefCell;
use embassy_executor::main;
use static_cell::StaticCell;
use mipidsi::{Builder, models::ILI9341Rgb565, options::{Orientation, Rotation}};


bind_interrupts!(struct Irqs {
    GPDMA1_CHANNEL0 => dma::InterruptHandler<peripherals::GPDMA1_CH0>;
    GPDMA1_CHANNEL1 => dma::InterruptHandler<peripherals::GPDMA1_CH1>;
    EXTI0 => embassy_stm32::exti::InterruptHandler<EXTI0>;
    EXTI1 => embassy_stm32::exti::InterruptHandler<EXTI1>;
    EXTI2 => embassy_stm32::exti::InterruptHandler<EXTI2>;
    EXTI3 => embassy_stm32::exti::InterruptHandler<EXTI3>;
    EXTI5 => embassy_stm32::exti::InterruptHandler<EXTI5>;
    EXTI6 => embassy_stm32::exti::InterruptHandler<EXTI6>;
    EXTI8 => embassy_stm32::exti::InterruptHandler<EXTI8>;
    EXTI10 => embassy_stm32::exti::InterruptHandler<EXTI10>;
    EXTI11 => embassy_stm32::exti::InterruptHandler<EXTI11>;
    EXTI12 => embassy_stm32::exti::InterruptHandler<EXTI12>;
    EXTI13 => embassy_stm32::exti::InterruptHandler<EXTI13>;
    EXTI14 => embassy_stm32::exti::InterruptHandler<EXTI14>;
    EXTI15 => embassy_stm32::exti::InterruptHandler<EXTI15>;
});

static STATE: StaticCell<Mutex<ThreadModeRawMutex, RefCell<SynthState>>> = StaticCell::new();
static VOLUME: AtomicI16 = AtomicI16::new(16000);
static DMA_BUF: StaticCell<[u16; 2048]> = StaticCell::new();

#[main]
async fn main(spawner: Spawner) {
    let mut mcu_config = embassy_stm32::Config::default();

    // base clk 16MHz
    mcu_config.rcc.hsi = true;

    // PLL1 PROCESSOR (80 MHz)
    mcu_config.rcc.pll1 = Some(embassy_stm32::rcc::Pll {
        source: embassy_stm32::rcc::PllSource::HSI,
        prediv: embassy_stm32::rcc::PllPreDiv::DIV2,
        mul: embassy_stm32::rcc::PllMul::MUL20,
        divp: None,
        divq: None,
        divr: Some(embassy_stm32::rcc::PllDiv::DIV2),
    });
    mcu_config.rcc.sys = embassy_stm32::rcc::Sysclk::PLL1_R;

    // PLL3 AUDIO (61.44 MHz)
    mcu_config.rcc.msis = Some(embassy_stm32::rcc::MSIRange::RANGE_48MHZ);
    mcu_config.rcc.pll3 = Some(embassy_stm32::rcc::Pll {
        source: embassy_stm32::rcc::PllSource::MSIS,
        prediv: embassy_stm32::rcc::PllPreDiv::DIV5,    //  9.6MHz
        mul: embassy_stm32::rcc::PllMul::MUL32,        // 307.2MHz
        divp: Some(embassy_stm32::rcc::PllDiv::DIV5), //   61.44MHz
        divq: None,
        divr: None,
    });
    mcu_config.rcc.mux.sai1sel = embassy_stm32::rcc::mux::Saisel::PLL3_P;

    let p = embassy_stm32::init(mcu_config);

    // dac init
    let mut sai_config = Config::default();
    sai_config.mode = Mode::Master;
    sai_config.tx_rx = TxRx::Transmitter;
    sai_config.protocol = Protocol::Free;
    sai_config.data_size = DataSize::Data16;
    sai_config.slot_count = word::U4(2);
    // both channels (3 = 0b00000011)
    sai_config.slot_enable = 3;
    // 16 left + 16 right
    sai_config.frame_length = 32;
    sai_config.frame_sync_active_level_length = word::U7(16);
    sai_config.frame_sync_offset = embassy_stm32::sai::FrameSyncOffset::BeforeFirstBit;
    sai_config.frame_sync_polarity = embassy_stm32::sai::FrameSyncPolarity::ActiveLow;
    sai_config.clock_strobe = embassy_stm32::sai::ClockStrobe::Falling;
    sai_config.bit_order = embassy_stm32::sai::BitOrder::MsbFirst;
    // get 48KHz
    sai_config.master_clock_divider = MasterClockDivider::DIV5;
    let sai1_subblocks = split_subblocks(p.SAI1);

    let dma_buf = DMA_BUF.init([0u16; 2048]);
    let sai = Sai::new_asynchronous(
        sai1_subblocks.0, // subblock A
        p.PA8,            // SCK -> BCK
        p.PA10,           // SD  -> DIN
        p.PA9,           // FS  -> LRCK
        p.GPDMA1_CH0,
        dma_buf,
        Irqs,
        sai_config,
    );

    // display init
    let cs = Output::new(p.PC9, Level::High, Speed::VeryHigh);
    let rst = Output::new(p.PC7, Level::High, Speed::VeryHigh);
    let dc = Output::new(p.PC6, Level::Low, Speed::VeryHigh);
    let mut led = Output::new(p.PA11, Level::Low, Speed::Low);
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(16_000_000);
    let spi = Spi::new_blocking_txonly(p.SPI1, p.PA5, p.PA12, spi_config);
    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
    let di = SPIInterface::new(spi_device, dc);
    let mut delay = Delay;
    let mut display = Builder::new(ILI9341Rgb565, di)
    .orientation(Orientation::new().rotate(Rotation::Deg90))
    .reset_pin(rst)
    .init(&mut delay)
    .unwrap();

    led.set_high();

    let adc = Adc::new(p.ADC1);
    let vol_pot = p.PC0.degrade_adc();
    let eff1_pot = p.PC1.degrade_adc();
    let eff2_pot = p.PC3.degrade_adc();
    let eff4_pot = p.PC2.degrade_adc();

    let note_a_sharp = ExtiInput::new(p.PC10, p.EXTI10, Pull::Up, Irqs);
    let note_g_sharp = ExtiInput::new(p.PC12, p.EXTI12, Pull::Up, Irqs);

    let note_c2 = ExtiInput::new(p.PC11, p.EXTI11, Pull::Up, Irqs);
    let note_b = ExtiInput::new(p.PD2, p.EXTI2, Pull::Up, Irqs);

    let note_f_sharp = ExtiInput::new(p.PC13, p.EXTI13, Pull::Up, Irqs);
    let note_d_sharp = ExtiInput::new(p.PB15, p.EXTI15, Pull::Up, Irqs);
    let note_c_sharp = ExtiInput::new(p.PB14, p.EXTI14, Pull::Up, Irqs);

    let note_d = ExtiInput::new(p.PH0, p.EXTI0, Pull::Up, Irqs);
    let note_c1 = ExtiInput::new(p.PB1, p.EXTI1, Pull::Up, Irqs);

    let note_a = ExtiInput::new(p.PA6, p.EXTI6, Pull::Up, Irqs);
    let note_g = ExtiInput::new(p.PB5, p.EXTI5, Pull::Up, Irqs);
    let note_f = ExtiInput::new(p.PB3, p.EXTI3, Pull::Up, Irqs);
    let note_e = ExtiInput::new(p.PC8, p.EXTI8, Pull::Up, Irqs);
    let keys= [
        note_c1, note_c_sharp, note_d, note_d_sharp, note_e, note_f, note_f_sharp, note_g, note_g_sharp, note_a, note_a_sharp, note_b, note_c2
    ];

    let mut qei_conf = qei::Config::default();
    qei_conf.ch1_pull = Pull::Up;
    qei_conf.ch2_pull = Pull::Up;
    let enc1 = Qei::new(p.TIM2, p.PA0, p.PA1, qei_conf);
    let enc2 = Qei::new(p.TIM3, p.PB4, p.PA7, qei_conf);

    let initial_state = SynthState::new();
    let state_ref = STATE.init(Mutex::new(RefCell::new(initial_state)));
    
    for (i, key) in keys.into_iter().enumerate() {
        spawner.spawn(key_task(state_ref, key, i)).unwrap();
    }

    spawner.spawn(input_task(state_ref, adc, p.GPDMA1_CH1,
        vol_pot, eff1_pot, eff2_pot, eff4_pot,
        enc1, enc2)).unwrap();

    let b_next_page = Input::new(p.PA4, Pull::Up);
    let b_prev_page = Input::new(p.PB0, Pull::Up);

    spawner.spawn(page_task(state_ref, b_next_page, 1)).unwrap();
    spawner.spawn(page_task(state_ref, b_prev_page, 0)).unwrap();

    let b_pause_play_clr = Input::new(p.PB13, Pull::Up);
    let b_rec = Input::new(p.PB10, Pull::Up);

    spawner.spawn(btn_play_task(state_ref, b_pause_play_clr)).unwrap();
    spawner.spawn(btn_rec_task(state_ref, b_rec)).unwrap();

    static SIN_WAVETABLE: StaticCell<[f32; 128]> = StaticCell::new();
    static TRI_WAVETABLE: StaticCell<[f32; 128]> = StaticCell::new();
    static SAW_WAVETABLE: StaticCell<[f32; 128]> = StaticCell::new();

    let sin_table = SIN_WAVETABLE.init(make_wtable(wave_sine));
    let tri_table = TRI_WAVETABLE.init(make_wtable(wave_triangle));
    let saw_table = SAW_WAVETABLE.init(make_wtable(wave_saw));

    spawner.spawn(audio_task(state_ref, sai, sin_table, tri_table, saw_table)).unwrap();

    display.clear(Rgb565::BLUE).unwrap();
    let mut ui = DisplayManager::new(display);
    loop {
        state_ref.lock(|s| {
            let synth = s.borrow();
            ui.render(&synth.presets, synth.idx, synth.page);
        });

        embassy_time::Timer::after_millis(33).await;
    }
}
