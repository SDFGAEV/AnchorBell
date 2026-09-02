#[derive(Debug, Clone, Copy)]
pub struct MakerPriceEngine {
    pub offset_bps: i64,
}

impl MakerPriceEngine {
    pub fn new(offset_bps: i64) -> Self {
        Self { offset_bps }
    }

    #[inline]
    pub fn buy_price(&self, index_price: i64) -> i64 {
        scaled_price(index_price, -i128::from(self.offset_bps))
    }

    #[inline]
    pub fn sell_price(&self, index_price: i64) -> i64 {
        scaled_price(index_price, i128::from(self.offset_bps))
    }
}

#[inline]
fn scaled_price(index_price: i64, offset_bps: i128) -> i64 {
    let adjustment = i128::from(index_price) * offset_bps / 10_000;
    (i128::from(index_price) + adjustment).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

#[cfg(test)]
mod tests {
    use super::MakerPriceEngine;

    #[test]
    fn applies_offset_in_bps() {
        let engine = MakerPriceEngine::new(100);
        assert_eq!(engine.buy_price(100_000), 99_000);
        assert_eq!(engine.sell_price(100_000), 101_000);
    }

    #[test]
    fn handles_extreme_prices_without_overflow() {
        let engine = MakerPriceEngine::new(i64::MAX);
        assert_eq!(engine.buy_price(i64::MAX), i64::MIN);
        assert_eq!(engine.sell_price(i64::MAX), i64::MAX);
    }
}
