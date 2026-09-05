use crate::simulation_runtime::SimulationPolicyVariant;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExperimentSpec {
    pub label: String,
    pub strategy: String,
    pub ablations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExperimentPlan {
    pub schema_version: u16,
    pub plan_id: String,
    pub experiments: Vec<ExperimentSpec>,
}

impl ExperimentPlan {
    pub const SCHEMA_VERSION: u16 = 1;

    pub fn m1_to_m8() -> Self {
        let names = [
            ("F1_m1", "m1"),
            ("F2_m2", "m2"),
            ("F3_m3", "m3"),
            ("F4_m4", "m4"),
            ("F5_m5", "m5"),
            ("F6_m6", "m6"),
            ("F7_m7", "m7"),
            ("M8_full", "m8"),
        ];
        let mut experiments = names
            .into_iter()
            .map(|(label, strategy)| ExperimentSpec {
                label: label.into(),
                strategy: strategy.into(),
                ablations: vec![],
            })
            .collect::<Vec<_>>();
        experiments.extend([
            ExperimentSpec {
                label: "M8_no_funding".into(),
                strategy: "m7".into(),
                ablations: vec!["funding".into()],
            },
            ExperimentSpec {
                label: "R7_m7".into(),
                strategy: "m7".into(),
                ablations: vec![],
            },
            ExperimentSpec {
                label: "R6_m6".into(),
                strategy: "m6".into(),
                ablations: vec![],
            },
            ExperimentSpec {
                label: "R5_m5".into(),
                strategy: "m5".into(),
                ablations: vec![],
            },
            ExperimentSpec {
                label: "R4_m4".into(),
                strategy: "m4".into(),
                ablations: vec![],
            },
            ExperimentSpec {
                label: "R3_m3".into(),
                strategy: "m3".into(),
                ablations: vec![],
            },
            ExperimentSpec {
                label: "R2_m2".into(),
                strategy: "m2".into(),
                ablations: vec![],
            },
            ExperimentSpec {
                label: "R1_m1".into(),
                strategy: "m1".into(),
                ablations: vec![],
            },
        ]);
        Self {
            schema_version: Self::SCHEMA_VERSION,
            plan_id: "m1-m8-ablation-matrix".into(),
            experiments,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != Self::SCHEMA_VERSION || self.plan_id.trim().is_empty() {
            return Err("invalid experiment plan identity");
        }
        if self.experiments.is_empty() || self.experiments.iter().any(|e| e.label.trim().is_empty())
        {
            return Err("experiment plan cannot be empty");
        }
        Ok(())
    }

    pub fn runtime_specs(&self) -> Result<Vec<(String, SimulationPolicyVariant)>, &'static str> {
        self.validate()?;
        self.experiments
            .iter()
            .map(|experiment| {
                let variant = match experiment.strategy.as_str() {
                    "m1" => SimulationPolicyVariant::M1AdaptiveRisk,
                    "m2" => SimulationPolicyVariant::M2Microstructure,
                    "m3" => SimulationPolicyVariant::M3FillAware,
                    "m4" => SimulationPolicyVariant::M4Statistical,
                    "m5" => SimulationPolicyVariant::M5Robust,
                    "m6" => SimulationPolicyVariant::M6DynamicCapital,
                    "m7" => SimulationPolicyVariant::M7EvidenceGated,
                    "m8" => SimulationPolicyVariant::M8FundingAware,
                    _ => return Err("unknown experiment strategy"),
                };
                Ok((experiment.label.clone(), variant))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_plan_contains_the_full_matrix() {
        let plan = ExperimentPlan::m1_to_m8();
        assert_eq!(plan.experiments.len(), 16);
        assert_eq!(plan.runtime_specs().unwrap().len(), 16);
    }
}
