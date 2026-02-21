use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke};
use iced::{mouse, Element, Length, Rectangle, Renderer, Theme};

use crate::message::Message;
use crate::theme;

/// Rolling data buffer for sparkline display.
#[derive(Debug, Clone)]
pub struct SparklineData {
    pub samples: Vec<f64>,
    pub max_samples: usize,
}

impl SparklineData {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::new(),
            max_samples,
        }
    }

    /// Push a new sample, trimming old ones beyond max_samples.
    pub fn push(&mut self, value: f64) {
        self.samples.push(value);
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }
    }

    /// Current (most recent) value, or 0.0 if empty.
    pub fn current(&self) -> f64 {
        self.samples.last().copied().unwrap_or(0.0)
    }

    /// Peak value in the buffer.
    pub fn peak(&self) -> f64 {
        self.samples
            .iter()
            .copied()
            .fold(0.0_f64, f64::max)
    }
}

/// Canvas program that draws the sparkline.
struct SparklineProgram {
    samples: Vec<f64>,
    line_color: iced::Color,
}

impl canvas::Program<Message> for SparklineProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        if self.samples.len() < 2 {
            return vec![frame.into_geometry()];
        }

        let peak = self
            .samples
            .iter()
            .copied()
            .fold(0.0_f64, f64::max);
        let y_max = f64::max(10.0, peak * 1.2);
        let w = bounds.width as f64;
        let h = bounds.height as f64;
        let n = self.samples.len();

        // Build polyline points
        let points: Vec<iced::Point> = self
            .samples
            .iter()
            .enumerate()
            .map(|(i, &val)| {
                let x = (i as f64 / (n - 1) as f64) * w;
                let y = h - (val / y_max) * h;
                iced::Point::new(x as f32, y as f32)
            })
            .collect();

        // Filled area below the line (12% opacity)
        if !points.is_empty() {
            let mut fill_builder = canvas::path::Builder::new();
            fill_builder.move_to(iced::Point::new(points[0].x, bounds.height));
            for &pt in &points {
                fill_builder.line_to(pt);
            }
            fill_builder.line_to(iced::Point::new(
                points.last().unwrap().x,
                bounds.height,
            ));
            fill_builder.close();
            let fill_path = fill_builder.build();
            frame.fill(
                &fill_path,
                iced::Color {
                    a: 0.12,
                    ..self.line_color
                },
            );
        }

        // Line stroke
        let line_path = Path::new(|builder| {
            if let Some(first) = points.first() {
                builder.move_to(*first);
                for &pt in &points[1..] {
                    builder.line_to(pt);
                }
            }
        });
        frame.stroke(
            &line_path,
            Stroke::default()
                .with_color(self.line_color)
                .with_width(1.5),
        );

        vec![frame.into_geometry()]
    }
}

/// Create a sparkline view element from sample data.
pub fn sparkline_view(samples: &[f64]) -> Element<'static, Message> {
    let program = SparklineProgram {
        samples: samples.to_vec(),
        line_color: theme::METRIC_TPS,
    };

    Canvas::new(program)
        .width(Length::Fixed(theme::SPARKLINE_WIDTH))
        .height(Length::Fixed(theme::SPARKLINE_HEIGHT))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparkline_data_push_and_trim() {
        let mut data = SparklineData::new(5);
        for i in 0..10 {
            data.push(i as f64);
        }
        assert_eq!(data.samples.len(), 5);
        assert_eq!(data.samples, vec![5.0, 6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn test_sparkline_data_current() {
        let mut data = SparklineData::new(10);
        assert_eq!(data.current(), 0.0);
        data.push(42.0);
        assert_eq!(data.current(), 42.0);
        data.push(7.0);
        assert_eq!(data.current(), 7.0);
    }

    #[test]
    fn test_sparkline_data_peak() {
        let mut data = SparklineData::new(10);
        assert_eq!(data.peak(), 0.0);
        data.push(5.0);
        data.push(15.0);
        data.push(3.0);
        assert_eq!(data.peak(), 15.0);
    }

    #[test]
    fn test_sparkline_data_empty() {
        let data = SparklineData::new(10);
        assert_eq!(data.samples.len(), 0);
        assert_eq!(data.current(), 0.0);
        assert_eq!(data.peak(), 0.0);
    }
}
