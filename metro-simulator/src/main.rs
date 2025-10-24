//! # Example: Hello world
//!
//! A simple hello world example displaying some primitive shapes and some text underneath.

use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};
use metro_render::scene::train_predictions::{TrainPrediction, TrainPredictions};
use u8g2_fonts::{Font, FontRenderer};

pub struct MetroFont;

impl Font for MetroFont {
    const DATA: &'static [u8] =
        include_bytes!("../../metro-font-builder/sample_output/pims_3.u8g2font");
}

fn main() -> Result<(), std::convert::Infallible> {
    // Create a new simulator display with 192x64 pixels.
    let mut display: SimulatorDisplay<Rgb888> = SimulatorDisplay::new(Size::new(192, 64));

    // Create white rectangle as background for entire display.

    // Draw centered text.
    let font = FontRenderer::new::<MetroFont>().with_line_height(16);

    // Build predictions as a slice of borrowed string slices to match no_std scene API
    let predictions: &[TrainPrediction] = &[
        TrainPrediction {
            line: "SV",
            cars: "8",
            destination: "Ashburn",
            arrival_in_minutes: "1",
        },
        TrainPrediction {
            line: "OR",
            cars: "6",
            destination: "Vienna",
            arrival_in_minutes: "3",
        },
        TrainPrediction {
            line: "BL",
            cars: "6",
            destination: "Franconia",
            arrival_in_minutes: "11",
        },
    ];

    let mut scene = TrainPredictions { predictions, font };
    scene.render(&mut display);

    let output_settings = OutputSettingsBuilder::new()
        .scale(4)
        .pixel_spacing(1)
        .build();
    Window::new("Hello World", &output_settings).show_static(&display);

    Ok(())
}
