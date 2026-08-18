#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DifficultyAdjustment {
    pub target_block_time: u64,
    pub adjustment_interval: u64,
    pub min_difficulty: u64,
    pub max_difficulty: u64,
}

impl DifficultyAdjustment {
    pub fn new(
        target_block_time: u64,
        adjustment_interval: u64,
        min_difficulty: u64,
        max_difficulty: u64,
    ) -> Self {
        Self {
            target_block_time,
            adjustment_interval,
            min_difficulty,
            max_difficulty,
        }
    }

    /// Calculates the difficulty expected at a difficulty-adjustment point.
    pub fn expected_difficulty(
        &self,
        current_difficulty: u64,
        previous_timestamp: u64,
        current_timestamp: u64,
        block_count: u64,
    ) -> Result<u64, String> {
        if self.target_block_time == 0 {
            return Err("target_block_time must be greater than zero".to_string());
        }

        if self.adjustment_interval == 0 {
            return Err("adjustment_interval must be greater than zero".to_string());
        }

        if self.min_difficulty > self.max_difficulty {
            return Err("min_difficulty cannot be greater than max_difficulty".to_string());
        }

        if current_timestamp <= previous_timestamp {
            return Err("current_timestamp must be greater than previous_timestamp".to_string());
        }

        if block_count == 0 {
            return Err("block_count must be greater than zero".to_string());
        }

        let elapsed = current_timestamp - previous_timestamp;
        let average_block_time = elapsed / block_count;

        if average_block_time == 0 {
            return Ok(self
                .max_difficulty
                .min(current_difficulty.max(self.min_difficulty)));
        }

        let next_difficulty = current_difficulty
            .saturating_mul(self.target_block_time)
            .saturating_div(average_block_time);

        Ok(next_difficulty.clamp(self.min_difficulty, self.max_difficulty))
    }

    /// Returns the difficulty expected for the next block.
    ///
    /// Difficulty changes only when the next block reaches
    /// the configured adjustment interval.
    pub fn expected_difficulty_for_height(
        &self,
        current_difficulty: u64,
        previous_timestamp: u64,
        adjustment_start_timestamp: u64,
        next_height: u64,
    ) -> Result<u64, String> {
        if self.adjustment_interval == 0 {
            return Err("adjustment_interval must be greater than zero".to_string());
        }

        if next_height == 0 {
            return Err("next_height must be greater than zero".to_string());
        }

        // No adjustment between intervals.
        if !next_height.is_multiple_of(self.adjustment_interval) {
            return Ok(current_difficulty.clamp(self.min_difficulty, self.max_difficulty));
        }

        let block_count = self.adjustment_interval;

        self.expected_difficulty(
            current_difficulty,
            adjustment_start_timestamp,
            previous_timestamp,
            block_count,
        )
    }
}

impl Default for DifficultyAdjustment {
    fn default() -> Self {
        Self::new(60, 2_016, 1, 32)
    }
}

#[cfg(test)]
mod tests {
    use super::DifficultyAdjustment;

    #[test]
    fn fast_blocks_increase_difficulty() {
        let adjustment = DifficultyAdjustment::new(60, 10, 1, 32);

        let result = adjustment.expected_difficulty(5, 1_000, 1_500, 10).unwrap();

        assert_eq!(result, 6);
    }

    #[test]
    fn slow_blocks_decrease_difficulty() {
        let adjustment = DifficultyAdjustment::new(60, 10, 1, 32);

        let result = adjustment.expected_difficulty(5, 1_000, 1_700, 10).unwrap();

        assert_eq!(result, 4);
    }

    #[test]
    fn difficulty_respects_minimum() {
        let adjustment = DifficultyAdjustment::new(60, 10, 1, 32);

        let result = adjustment.expected_difficulty(1, 1_000, 1_700, 10).unwrap();

        assert_eq!(result, 1);
    }

    #[test]
    fn difficulty_respects_maximum() {
        let adjustment = DifficultyAdjustment::new(60, 10, 1, 32);

        let result = adjustment
            .expected_difficulty(32, 1_000, 1_100, 10)
            .unwrap();

        assert_eq!(result, 32);
    }

    #[test]
    fn zero_target_time_is_rejected() {
        let adjustment = DifficultyAdjustment::new(0, 10, 1, 32);

        assert!(adjustment.expected_difficulty(5, 1_000, 1_100, 10).is_err());
    }

    #[test]
    fn zero_interval_is_rejected() {
        let adjustment = DifficultyAdjustment::new(60, 0, 1, 32);

        assert!(adjustment.expected_difficulty(5, 1_000, 1_100, 10).is_err());
    }

    #[test]
    fn invalid_range_is_rejected() {
        let adjustment = DifficultyAdjustment::new(60, 10, 10, 5);

        assert!(adjustment
            .expected_difficulty(10, 1_000, 1_100, 10)
            .is_err());
    }

    #[test]
    fn reversed_timestamps_are_rejected() {
        let adjustment = DifficultyAdjustment::new(60, 10, 1, 32);

        assert!(adjustment.expected_difficulty(5, 2_000, 1_000, 10).is_err());
    }

    #[test]
    fn difficulty_does_not_change_before_interval() {
        let adjustment = DifficultyAdjustment::new(60, 10, 1, 32);

        let result = adjustment
            .expected_difficulty_for_height(5, 1_500, 1_000, 9)
            .unwrap();

        assert_eq!(result, 5);
    }

    #[test]
    fn difficulty_adjusts_at_interval() {
        let adjustment = DifficultyAdjustment::new(60, 10, 1, 32);

        let result = adjustment
            .expected_difficulty_for_height(5, 1_500, 1_000, 10)
            .unwrap();

        assert_eq!(result, 6);
    }

    #[test]
    fn default_configuration_is_valid() {
        let adjustment = DifficultyAdjustment::default();

        assert_eq!(adjustment.target_block_time, 60);
        assert_eq!(adjustment.adjustment_interval, 2_016);
        assert_eq!(adjustment.min_difficulty, 1);
        assert_eq!(adjustment.max_difficulty, 32);
    }
}
