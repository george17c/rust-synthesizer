#![no_std]
#![no_main]

mod types;
mod audio;
mod input;

// use types::Pots;
use audio::audio_task;
use input::input_task;

use core::sync::atomic::{AtomicI16};
use embassy_executor::Spawner;
use {defmt_rtt as _, panic_probe as _};
use embassy_stm32::{
    adc::AdcChannel, bind_interrupts, dma, gpio::{Level, Output, Speed},
    interrupt::typelevel::EXTI13, peripherals,
    sai::{Config, DataSize, MasterClockDivider, Mode, Protocol, Sai, TxRx, split_subblocks, word},
    spi::{Config as SpiConfig, Spi}, time::Hertz,
};
use embassy_stm32::adc::{Adc};
use embassy_time::Delay;
use display_interface_spi::SPIInterface;
use embedded_hal_bus::{
    spi::{ExclusiveDevice},
};
use static_cell::StaticCell;
use mipidsi::{Builder, models::ILI9341Rgb565, options::{Orientation, Rotation}};


bind_interrupts!(struct Irqs {
    GPDMA1_CHANNEL0 => dma::InterruptHandler<peripherals::GPDMA1_CH0>;
    GPDMA1_CHANNEL1 => dma::InterruptHandler<peripherals::GPDMA1_CH1>;
    EXTI13 => embassy_stm32::exti::InterruptHandler<EXTI13>;
});

static VOLUME: AtomicI16 = AtomicI16::new(16000);
static DMA_BUF: StaticCell<[u16; 1024]> = StaticCell::new();

#[embassy_executor::main]
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

    // PLL3 AUDIO (15.36 MHz)
    mcu_config.rcc.pll3 = Some(embassy_stm32::rcc::Pll {
        source: embassy_stm32::rcc::PllSource::HSI,
        prediv: embassy_stm32::rcc::PllPreDiv::DIV2,
        mul: embassy_stm32::rcc::PllMul::MUL48,
        divp: Some(embassy_stm32::rcc::PllDiv::DIV25),
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
    // Dăm voie la ambele canale (3 = 0b00000011)
    sai_config.slot_enable = 3;
    // Un cadru total e 32 (16 stanga + 16 dreapta) 
    sai_config.frame_length = 32;
    sai_config.frame_sync_active_level_length = word::U7(16);
    // get 48KHz from 15.36MHz
    sai_config.master_clock_divider = MasterClockDivider::DIV5;
    let sai1_subblocks = split_subblocks(p.SAI1);

    let dma_buf = DMA_BUF.init([0u16; 1024]);
    let sai = Sai::new_asynchronous(
        sai1_subblocks.0, // Folosim Sub-blocul A
        p.PA8,            // SCK -> BCK
        p.PA10,           // SD  -> DIN
        p.PA9,            // FS  -> LRCK
        p.GPDMA1_CH0,
        dma_buf,
        Irqs,
        sai_config,
    );

    // display init
    let cs = Output::new(p.PC9, Level::High, Speed::VeryHigh);
    let dc = Output::new(p.PC6, Level::Low, Speed::VeryHigh);
    let mut led = Output::new(p.PA6, Level::Low, Speed::Low);
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(16_000_000);
    let spi = Spi::new_blocking_txonly(p.SPI1, p.PA5, p.PA7, spi_config);
    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
    let di = SPIInterface::new(spi_device, dc);
    let mut delay = Delay;
    let mut _display = Builder::new(ILI9341Rgb565, di)
    .orientation(Orientation::new().rotate(Rotation::Deg90))
    .init(&mut delay)
    .unwrap();

    led.set_high();

    let adc = Adc::new(p.ADC1);
    let vol_pot = p.PA4.degrade_adc();
    let dummy1 = p.PC0.degrade_adc();
    let dummy2 = p.PC1.degrade_adc();

    spawner.spawn(audio_task(sai)).unwrap();
    spawner.spawn(input_task(adc, p.GPDMA1_CH1, vol_pot, dummy1, dummy2)).unwrap();

    core::future::pending::<()>().await;
}
