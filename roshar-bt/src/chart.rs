use plotters::prelude::*;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::performance::PerformanceMetrics;

#[derive(Debug, Clone)]
pub struct TimeSeriesPoint {
    pub timestamp: i64,
    pub price: Decimal,
    pub position: Decimal,
    pub cumulative_return: Decimal,
}

pub struct ChartData {
    pub data_points: Vec<TimeSeriesPoint>,
}

impl ChartData {
    pub fn new() -> Self {
        Self {
            data_points: Vec::new(),
        }
    }

    pub fn add_point(&mut self, timestamp: i64, price: Decimal, position: Decimal, cumulative_return: Decimal) {
        self.data_points.push(TimeSeriesPoint {
            timestamp,
            price,
            position,
            cumulative_return,
        });
    }

    pub fn from_performance_metrics(performance: &PerformanceMetrics) -> Self {
        let mut chart_data = ChartData::new();
        let positions = performance.get_position_history();
        let prices = performance.get_prices();
        let cumulative_returns = performance.get_cumulative_returns_history();
        
        for (i, &(timestamp, price)) in prices.iter().enumerate() {
            let position = if i < positions.len() {
                positions[i].1
            } else {
                positions.back().map_or(Decimal::ZERO, |p| p.1)
            };
            
            let cumulative_return = if i < cumulative_returns.len() {
                cumulative_returns[i]
            } else {
                Decimal::ZERO
            };
            
            chart_data.add_point(timestamp, price, position, cumulative_return);
        }
        
        chart_data
    }

    pub fn create_chart(&self, output_path: &str, title: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.data_points.is_empty() {
            return Err("No data points to chart".into());
        }

        let root = BitMapBackend::new(output_path, (1200, 800)).into_drawing_area();
        root.fill(&WHITE)?;

        let mut chart = ChartBuilder::on(&root)
            .caption(title, ("sans-serif", 40))
            .margin(10)
            .x_label_area_size(60)
            .y_label_area_size(80)
            .build_cartesian_2d(
                self.get_time_range(),
                self.get_price_range(),
            )?;

        chart
            .configure_mesh()
            .x_desc("Time")
            .y_desc("Price")
            .draw()?;

        let price_data: Vec<(i64, f64)> = self.data_points
            .iter()
            .map(|p| (p.timestamp, p.price.to_f64().unwrap_or(0.0)))
            .collect();

        chart
            .draw_series(LineSeries::new(price_data, &BLUE))?
            .label("Price")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 10, y)], &BLUE));

        chart.configure_series_labels().draw()?;

        root.present()?;
        Ok(())
    }

    pub fn create_multi_chart(&self, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.data_points.is_empty() {
            return Err("No data points to chart".into());
        }

        let root = BitMapBackend::new(output_path, (1200, 1200)).into_drawing_area();
        root.fill(&WHITE)?;

        let charts = root.split_evenly((2, 1));

        self.draw_price_cumulative_return_overlay_chart(&charts[0])?;
        self.draw_position_chart(&charts[1])?;

        root.present()?;
        Ok(())
    }


    fn draw_price_cumulative_return_overlay_chart(&self, area: &DrawingArea<BitMapBackend, plotters::coord::Shift>) -> Result<(), Box<dyn std::error::Error>> {
        let mut chart = ChartBuilder::on(area)
            .caption("Price & Cumulative Return", ("sans-serif", 30))
            .margin(5)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .right_y_label_area_size(60)
            .build_cartesian_2d(
                self.get_time_range(),
                self.get_price_range(),
            )?
            .set_secondary_coord(
                self.get_time_range(),
                self.get_cumulative_return_range(),
            );

        chart
            .configure_mesh()
            .x_desc("Time")
            .y_desc("Price")
            .draw()?;

        chart
            .configure_secondary_axes()
            .y_desc("Cumulative Return")
            .draw()?;

        let price_data: Vec<(i64, f64)> = self.data_points
            .iter()
            .map(|p| (p.timestamp, p.price.to_f64().unwrap_or(0.0)))
            .collect();

        let return_data: Vec<(i64, f64)> = self.data_points
            .iter()
            .map(|p| (p.timestamp, p.cumulative_return.to_f64().unwrap_or(0.0)))
            .collect();

        chart.draw_series(LineSeries::new(price_data, &BLUE))?
            .label("Price")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 10, y)], &BLUE));

        chart.draw_secondary_series(LineSeries::new(return_data, &GREEN))?
            .label("Cumulative Return")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 10, y)], &GREEN));

        chart.configure_series_labels().draw()?;

        Ok(())
    }

    fn draw_position_chart(&self, area: &DrawingArea<BitMapBackend, plotters::coord::Shift>) -> Result<(), Box<dyn std::error::Error>> {
        let mut chart = ChartBuilder::on(area)
            .caption("Position", ("sans-serif", 30))
            .margin(5)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(
                self.get_time_range(),
                self.get_position_range(),
            )?;

        chart
            .configure_mesh()
            .x_desc("Time")
            .y_desc("Position Size")
            .draw()?;

        let position_data: Vec<(i64, f64)> = self.data_points
            .iter()
            .map(|p| (p.timestamp, p.position.to_f64().unwrap_or(0.0)))
            .collect();

        chart.draw_series(LineSeries::new(position_data, &RED))?
            .label("Position")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 10, y)], &RED));

        let zero_line: Vec<(i64, f64)> = vec![
            (self.data_points.first().unwrap().timestamp, 0.0),
            (self.data_points.last().unwrap().timestamp, 0.0),
        ];
        chart.draw_series(LineSeries::new(zero_line, &BLACK.mix(0.3)))?;

        chart.configure_series_labels().draw()?;

        Ok(())
    }

    fn get_time_range(&self) -> std::ops::Range<i64> {
        let min_time = self.data_points.iter().map(|p| p.timestamp).min().unwrap_or(0);
        let max_time = self.data_points.iter().map(|p| p.timestamp).max().unwrap_or(1);
        min_time..max_time
    }

    fn get_price_range(&self) -> std::ops::Range<f64> {
        let prices: Vec<f64> = self.data_points
            .iter()
            .map(|p| p.price.to_f64().unwrap_or(0.0))
            .collect();
        let min_price = prices.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_price = prices.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let padding = (max_price - min_price) * 0.05;
        (min_price - padding)..(max_price + padding)
    }

    fn get_cumulative_return_range(&self) -> std::ops::Range<f64> {
        let returns: Vec<f64> = self.data_points
            .iter()
            .map(|p| p.cumulative_return.to_f64().unwrap_or(0.0))
            .collect();
        let min_return = returns.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_return = returns.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let padding = (max_return - min_return).abs() * 0.05;
        (min_return - padding)..(max_return + padding)
    }

    fn get_position_range(&self) -> std::ops::Range<f64> {
        let positions: Vec<f64> = self.data_points
            .iter()
            .map(|p| p.position.to_f64().unwrap_or(0.0))
            .collect();
        let min_pos = positions.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_pos = positions.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let padding = (max_pos - min_pos).abs() * 0.1;
        (min_pos - padding)..(max_pos + padding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn test_chart_data_creation() {
        let mut chart_data = ChartData::new();
        chart_data.add_point(1000, dec!(100), dec!(10), dec!(0.01));
        chart_data.add_point(1001, dec!(101), dec!(5), dec!(0.02));
        
        assert_eq!(chart_data.data_points.len(), 2);
        assert_eq!(chart_data.data_points[0].price, dec!(100));
        assert_eq!(chart_data.data_points[1].position, dec!(5));
    }

    #[test]
    fn test_time_range() {
        let mut chart_data = ChartData::new();
        chart_data.add_point(1000, dec!(100), dec!(10), dec!(0.01));
        chart_data.add_point(2000, dec!(101), dec!(5), dec!(0.02));
        chart_data.add_point(1500, dec!(99), dec!(15), dec!(0.015));
        
        let range = chart_data.get_time_range();
        assert_eq!(range.start, 1000);
        assert_eq!(range.end, 2000);
    }
}
