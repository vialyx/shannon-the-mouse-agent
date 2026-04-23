use serde::{Deserialize, Serialize};
use std::fmt;

/// Qualitative risk classification derived from the numeric risk score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "LOW"),
            RiskLevel::Medium => write!(f, "MEDIUM"),
            RiskLevel::High => write!(f, "HIGH"),
            RiskLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Maps a continuous risk score to a named threshold level.
pub struct Scorer {
    pub medium: f64,
    pub high: f64,
    pub critical: f64,
}

impl Scorer {
    /// Classify a score in `[0.0, 1.0]` into a risk level.
    ///
    /// ```
    /// # use mouse_entropy_agent::scorer::{Scorer, RiskLevel};
    /// let s = Scorer { medium: 0.3, high: 0.6, critical: 0.8 };
    /// assert_eq!(s.classify(0.1), RiskLevel::Low);
    /// assert_eq!(s.classify(0.4), RiskLevel::Medium);
    /// assert_eq!(s.classify(0.7), RiskLevel::High);
    /// assert_eq!(s.classify(0.9), RiskLevel::Critical);
    /// ```
    pub fn classify(&self, score: f64) -> RiskLevel {
        if score >= self.critical {
            RiskLevel::Critical
        } else if score >= self.high {
            RiskLevel::High
        } else if score >= self.medium {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scorer() -> Scorer {
        Scorer {
            medium: 0.3,
            high: 0.6,
            critical: 0.8,
        }
    }

    #[test]
    fn boundary_low() {
        assert_eq!(scorer().classify(0.0), RiskLevel::Low);
        assert_eq!(scorer().classify(0.299), RiskLevel::Low);
    }

    #[test]
    fn boundary_medium() {
        assert_eq!(scorer().classify(0.3), RiskLevel::Medium);
        assert_eq!(scorer().classify(0.599), RiskLevel::Medium);
    }

    #[test]
    fn boundary_high() {
        assert_eq!(scorer().classify(0.6), RiskLevel::High);
        assert_eq!(scorer().classify(0.799), RiskLevel::High);
    }

    #[test]
    fn boundary_critical() {
        assert_eq!(scorer().classify(0.8), RiskLevel::Critical);
        assert_eq!(scorer().classify(1.0), RiskLevel::Critical);
    }

    #[test]
    fn display_strings() {
        assert_eq!(RiskLevel::Low.to_string(), "LOW");
        assert_eq!(RiskLevel::Medium.to_string(), "MEDIUM");
        assert_eq!(RiskLevel::High.to_string(), "HIGH");
        assert_eq!(RiskLevel::Critical.to_string(), "CRITICAL");
    }
}
