use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::MathematicalOps;
use std::collections::VecDeque;
use std::time::Duration;

pub struct PerformanceMetrics {
    prices: VecDeque<(i64, Decimal)>,
    positions: VecDeque<(i64, Decimal)>,
    returns: VecDeque<Decimal>,
    cumulative_returns: VecDeque<Decimal>,
    cumulative_return: Decimal,
    last_price: Option<Decimal>,
    last_position: Decimal,
    risk_free_rate: Decimal,
    return_window: Duration,
}

impl PerformanceMetrics {
    pub fn new(risk_free_rate: Decimal, return_window: Duration) -> Self {
        Self {
            prices: VecDeque::new(),
            positions: VecDeque::new(),
            returns: VecDeque::new(),
            cumulative_returns: VecDeque::new(),
            cumulative_return: Decimal::ZERO,
            last_price: None,
            last_position: Decimal::ZERO,
            risk_free_rate,
            return_window,
        }
    }

    pub fn update(&mut self, timestamp: i64, position: Decimal, mid_price: Decimal) {
        self.prices.push_back((timestamp, mid_price));
        self.positions.push_back((timestamp, position));
        
        // For the first update, cumulative return is 0
        if self.last_price.is_none() {
            self.cumulative_returns.push_back(Decimal::ZERO);
        } else if let Some(last_price) = self.last_price {
            let price_return = (mid_price - last_price) / last_price;
            let position_return = self.last_position * price_return;
            self.returns.push_back(position_return);
            self.cumulative_return += position_return;
            self.cumulative_returns.push_back(self.cumulative_return);
        }
        
        self.last_price = Some(mid_price);
        self.last_position = position;
    }

    pub fn calculate_sharpe_ratio(&self) -> Decimal {
        if self.returns.is_empty() {
            return Decimal::ZERO;
        }
        let mean_return = self.returns.iter().sum::<Decimal>() / Decimal::from(self.returns.len());
        let variance = self
            .returns
            .iter()
            .map(|r| (r - mean_return).powd(Decimal::TWO))
            .sum::<Decimal>()
            / Decimal::from(self.returns.len());
        let std_dev = variance.sqrt().unwrap_or(Decimal::ZERO);
        if std_dev == Decimal::ZERO {
            return Decimal::ZERO;
        }
        let annualization_factor =
            Decimal::from_f64((365.0 * 24.0 * 60.0 * 60.0) / self.return_window.as_secs_f64())
                .unwrap_or(Decimal::ONE);
        let annualized_mean = mean_return * annualization_factor;
        let annualized_std = std_dev * annualization_factor.sqrt().unwrap_or(Decimal::ONE);
        (annualized_mean - self.risk_free_rate) / annualized_std
    }

    pub fn get_cumulative_return(&self) -> Decimal {
        self.cumulative_return
    }

    pub fn get_position_history(&self) -> &VecDeque<(i64, Decimal)> {
        &self.positions
    }

    pub fn get_prices(&self) -> &VecDeque<(i64, Decimal)> {
        &self.prices
    }

    pub fn get_returns_history(&self) -> &VecDeque<Decimal> {
        &self.returns
    }
    
    pub fn get_cumulative_returns_history(&self) -> &VecDeque<Decimal> {
        &self.cumulative_returns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn test_position_tracking() {
        let mut metrics =
            PerformanceMetrics::new(Decimal::from_f64(0.02).unwrap(), Duration::from_secs(1));

        // Test initial state
        assert_eq!(metrics.get_position_history().len(), 0);

        // Test position updates
        metrics.update(1000, dec!(10), dec!(100));
        metrics.update(2000, dec!(-5), dec!(101));
        metrics.update(3000, dec!(0), dec!(102));

        let positions = metrics.get_position_history();
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], (1000, dec!(10)));
        assert_eq!(positions[1], (2000, dec!(-5)));
        assert_eq!(positions[2], (3000, dec!(0)));
    }

    #[test]
    fn test_returns_calculation() {
        let mut metrics =
            PerformanceMetrics::new(Decimal::from_f64(0.02).unwrap(), Duration::from_secs(1));

        // Test initial state
        assert_eq!(metrics.get_returns_history().len(), 0);
        assert_eq!(metrics.get_cumulative_return(), Decimal::ZERO);

        // Test returns calculation with position changes
        // The implementation calculates: position_return = last_position * price_return
        // This represents the P&L from holding last_position while price changes
        metrics.update(1000, dec!(10), dec!(100)); // Initial position
        metrics.update(2000, dec!(10), dec!(101)); // Price up 1%, position = 10
        metrics.update(3000, dec!(5), dec!(102)); // Price up ~0.99%, position = 10
        metrics.update(4000, dec!(0), dec!(101)); // Price down ~0.98%, position = 5

        let returns = metrics.get_returns_history();
        assert_eq!(returns.len(), 3); // 3 returns for 4 updates

        // First return: last_position (10) * price_return ((101-100)/100 = 0.01) = 0.1
        let price_return_1 = (dec!(101) - dec!(100)) / dec!(100);
        let expected_1 = dec!(10) * price_return_1;
        assert!((returns[0] - expected_1).abs() < dec!(0.0001));

        // Second return: last_position (10) * price_return ((102-101)/101)
        let price_return_2 = (dec!(102) - dec!(101)) / dec!(101);
        let expected_2 = dec!(10) * price_return_2;
        assert!((returns[1] - expected_2).abs() < dec!(0.0001));

        // Third return: last_position (5) * price_return ((101-102)/102)
        let price_return_3 = (dec!(101) - dec!(102)) / dec!(102);
        let expected_3 = dec!(5) * price_return_3;
        assert!((returns[2] - expected_3).abs() < dec!(0.0001));
    }

    #[test]
    fn test_sharpe_ratio_calculation() {
        let mut metrics = PerformanceMetrics::new(
            Decimal::from_f64(0.02).unwrap(), // 2% risk-free rate
            Duration::from_secs(1),
        );

        // Test empty returns
        assert_eq!(metrics.calculate_sharpe_ratio(), Decimal::ZERO);

        // Test with constant positive returns
        for i in 0..10 {
            metrics.update(
                i * 1000,
                dec!(10),                                     // Constant position
                Decimal::from_f64(100.0 + i as f64).unwrap(), // Increasing price
            );
        }

        let sharpe = metrics.calculate_sharpe_ratio();
        assert!(sharpe > Decimal::ZERO); // Should be positive with constant positive returns

        // Test with zero returns
        let mut metrics =
            PerformanceMetrics::new(Decimal::from_f64(0.02).unwrap(), Duration::from_secs(1));
        for i in 0..10 {
            metrics.update(
                i * 1000,
                dec!(0),   // No position
                dec!(100), // Constant price
            );
        }
        assert_eq!(metrics.calculate_sharpe_ratio(), Decimal::ZERO);

        // Test with alternating returns
        let mut metrics =
            PerformanceMetrics::new(Decimal::from_f64(0.02).unwrap(), Duration::from_secs(1));
        for i in 0..10 {
            metrics.update(
                i * 1000,
                dec!(10),                                      // Constant position
                if i % 2 == 0 { dec!(101) } else { dec!(99) }, // Alternating price
            );
        }
        let sharpe = metrics.calculate_sharpe_ratio();
        assert!(sharpe < Decimal::ZERO); // Should be negative with high volatility
    }

    #[test]
    fn test_annualization_factor() {
        let mut metrics = PerformanceMetrics::new(
            Decimal::from_f64(0.02).unwrap(),
            Duration::from_secs(3600), // 1-hour window
        );

        // Add some returns
        for i in 0..10 {
            metrics.update(
                i * 3600, // Hourly updates
                dec!(10),
                Decimal::from_f64(100.0 + i as f64).unwrap(),
            );
        }

        let sharpe = metrics.calculate_sharpe_ratio();
        // The annualization factor should be sqrt(24*365) for hourly returns
        // This is a basic check that the factor is being applied
        assert!(sharpe.abs() > Decimal::from_f64(0.1).unwrap());
    }
}
