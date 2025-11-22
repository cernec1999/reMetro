use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point, Size},
    primitives::Rectangle,
};
use u8g2_fonts::{
    FontRenderer,
    types::{FontColor, HorizontalAlignment, VerticalPosition},
};

const LN_X: u32 = 1;
const CAR_X: u32 = 33;
const CAR_NUM_X: u32 = 37;
const DEST_X: u32 = 78;
const DEST_NUM_X: u32 = 62;

const COLOR_YELLOW: Rgb888 = Rgb888::new(206, 154, 8);
const COLOR_RED: Rgb888 = Rgb888::new(255, 0, 0);
const COLOR_GREEN: Rgb888 = Rgb888::new(0, 255, 0);

pub struct TrainPredictions<'a> {
    pub predictions: &'a [TrainPrediction<'a>],
    pub font: FontRenderer,
}

#[derive(Copy, Clone)]
pub struct TrainPrediction<'a> {
    pub line: &'a str,
    pub cars: &'a str,
    pub destination: &'a str,
    pub arrival_in_minutes: &'a str,
}

impl<'a> TrainPredictions<'a> {
    pub fn render_header<Display>(&mut self, display: &mut Display)
    where
        Display: DrawTarget<Color = Rgb888>,
        Display::Error: core::fmt::Debug,
    {
        self.font
            .render_aligned(
                "LN",
                Point::new(LN_X as i32, 0),
                VerticalPosition::Top,
                HorizontalAlignment::Left,
                FontColor::Transparent(COLOR_RED),
                display,
            )
            .unwrap();

        self.font
            .render_aligned(
                "CAR",
                Point::new(CAR_X as i32, 0),
                VerticalPosition::Top,
                HorizontalAlignment::Left,
                FontColor::Transparent(COLOR_RED),
                display,
            )
            .unwrap();

        self.font
            .render_aligned(
                "DEST",
                Point::new(DEST_X as i32, 0),
                VerticalPosition::Top,
                HorizontalAlignment::Left,
                FontColor::Transparent(COLOR_RED),
                display,
            )
            .unwrap();

        // MIN is right aligned
        self.font
            .render_aligned(
                "MIN",
                Point::new(191, 0),
                VerticalPosition::Top,
                HorizontalAlignment::Right,
                FontColor::Transparent(COLOR_RED),
                display,
            )
            .unwrap();
    }

    pub fn render_train_row<Display>(
        &mut self,
        prediction: &TrainPrediction,
        row: u32,
        display: &mut Display,
    ) where
        Display: DrawTarget<Color = Rgb888>,
        Display::Error: core::fmt::Debug,
    {
        let y = (row as i32 + 1) * 16;

        self.font
            .render_aligned(
                prediction.line,
                Point::new(LN_X as i32, y),
                VerticalPosition::Top,
                HorizontalAlignment::Left,
                FontColor::Transparent(COLOR_YELLOW),
                display,
            )
            .unwrap();

        // if prediction.cars.as_str() == "8" we want COLOR_GREEN
        let color = match prediction.cars {
            "8" => COLOR_GREEN,
            "6" => COLOR_YELLOW,
            _ => COLOR_RED,
        };

        self.font
            .render_aligned(
                prediction.cars,
                Point::new(CAR_NUM_X as i32, y),
                VerticalPosition::Top,
                HorizontalAlignment::Left,
                FontColor::Transparent(color),
                display,
            )
            .unwrap();

        self.font
            .render_aligned(
                prediction.destination,
                Point::new(DEST_NUM_X as i32, y),
                VerticalPosition::Top,
                HorizontalAlignment::Left,
                FontColor::Transparent(COLOR_YELLOW),
                display,
            )
            .unwrap();

        // Arrival time is right aligned
        self.font
            .render_aligned(
                prediction.arrival_in_minutes,
                Point::new(191, y),
                VerticalPosition::Top,
                HorizontalAlignment::Right,
                FontColor::Transparent(COLOR_YELLOW),
                display,
            )
            .unwrap();
    }

    pub fn render<Display>(&mut self, display: &mut Display)
    where
        Display: DrawTarget<Color = Rgb888>,
        Display::Error: core::fmt::Debug,
    {
        self.render_header(display);

        // ensure that there are only up to 3 predictions
        for (i, prediction) in self.predictions.iter().take(3).enumerate() {
            self.render_train_row(prediction, i as u32, display);
        }

        // Render one point at 0, 63 to show bottom left corner is working
        display
            .fill_solid(
                &Rectangle::new(Point::new(0, 63), Size::new(1, 1)),
                Rgb888::new(255, 255, 255),
            )
            .unwrap();
    }
}
