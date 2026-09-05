use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Architectural plane owned by a subsystem.
///
/// Planes are dependency-ordered. A lower plane may not call a higher plane
/// through an untyped side channel; communication is via typed contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PlatformLayer {
    Control,
    MarketData,
    Decision,
    Execution,
    Simulation,
    Analytics,
    Observability,
}

/// Runtime authority for a system's decisions or data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Authority {
    Binance,
    ExternalReference,
    Derived,
    Operator,
    Internal,
}

/// Whether a component can be replaced without changing the safety kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mutability {
    ImmutableCore,
    GovernedPolicy,
    RuntimeState,
}

/// Canonical system role. This is intentionally operational vocabulary; analysis
/// and historical validation are consumers of the platform, not the platform
/// authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemRole {
    Registry,
    ExchangeAdapter,
    ReferenceData,
    Anchor,
    Strategy,
    Portfolio,
    Risk,
    Funding,
    ExecutionGateway,
    Lifecycle,
    Simulation,
    Replay,
    Backtest,
    Observability,
    Audit,
    ControlConsole,
    Recovery,
    Analytics,
}

/// Lifecycle/health state reported by every registered system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemState {
    Discovered,
    Ready,
    Degraded,
    Halted,
    Draining,
}

/// Compile-time identity and dependency contract for one subsystem.
#[derive(Debug, Clone, Serialize)]
pub struct SystemDescriptor {
    pub id: &'static str,
    pub layer: PlatformLayer,
    pub role: SystemRole,
    pub authority: Authority,
    pub mutability: Mutability,
    pub dependencies: &'static [&'static str],
    pub health_interval_ms: u64,
    pub restartable: bool,
}

/// Recovery behavior is part of the system contract, not an operator checklist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryPolicy {
    Halt,
    RestartThenReconcile,
    ReconcileThenResume,
    OperatorOnly,
}

/// The executable contract exposed by a registered system node.
#[derive(Debug, Clone, Serialize)]
pub struct SystemContract {
    pub system_id: &'static str,
    pub requires: &'static [&'static str],
    pub provides: &'static [&'static str],
    pub recovery: RecoveryPolicy,
}

impl SystemDescriptor {
    pub fn contract(&self) -> SystemContract {
        SystemContract {
            system_id: self.id,
            requires: self.dependencies,
            provides: capabilities_for(self.role),
            recovery: recovery_for(self.role, self.restartable),
        }
    }
}

fn capabilities_for(role: SystemRole) -> &'static [&'static str] {
    match role {
        SystemRole::Registry => &["system.discovery", "system.topology"],
        SystemRole::ExchangeAdapter => &["market.events", "market.connection"],
        SystemRole::ReferenceData => &["reference.fx", "reference.metadata"],
        SystemRole::Anchor => &["anchor.snapshot"],
        SystemRole::Strategy => &["decision.intent"],
        SystemRole::Portfolio => &["decision.allocation"],
        SystemRole::Risk => &["risk.admission", "risk.flatten"],
        SystemRole::Funding => &["funding.schedule", "funding.deadline"],
        SystemRole::ExecutionGateway => &["execution.submit", "execution.cancel"],
        SystemRole::Lifecycle => &["execution.lifecycle", "execution.reconcile"],
        SystemRole::Simulation => &["simulation.run"],
        SystemRole::Replay => &["simulation.replay"],
        SystemRole::Backtest => &["simulation.validation"],
        SystemRole::Observability => &["telemetry.health"],
        SystemRole::Audit => &["audit.events"],
        SystemRole::ControlConsole => &["control.operations"],
        SystemRole::Recovery => &["recovery.orchestration"],
        SystemRole::Analytics => &["analytics.evidence"],
    }
}

fn recovery_for(role: SystemRole, restartable: bool) -> RecoveryPolicy {
    if matches!(
        role,
        SystemRole::Registry | SystemRole::Risk | SystemRole::Recovery
    ) {
        RecoveryPolicy::Halt
    } else if matches!(role, SystemRole::ControlConsole) {
        RecoveryPolicy::OperatorOnly
    } else if restartable {
        RecoveryPolicy::RestartThenReconcile
    } else {
        RecoveryPolicy::ReconcileThenResume
    }
}

/// Runtime health signal. Producers update this asynchronously; decision and
/// execution paths only consume the last validated snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub system_id: String,
    pub state: SystemState,
    pub observed_at_ms: u64,
    pub stale: bool,
    pub invariant_failures: u32,
    pub queue_depth: u64,
    pub error_rate_ppm: u64,
    pub diagnostics: Vec<String>,
}

impl HealthSnapshot {
    pub fn discovered(system_id: impl Into<String>, observed_at_ms: u64) -> Self {
        Self {
            system_id: system_id.into(),
            state: SystemState::Discovered,
            observed_at_ms,
            stale: true,
            invariant_failures: 0,
            queue_depth: 0,
            error_rate_ppm: 0,
            diagnostics: vec!["awaiting_first_health_report".to_owned()],
        }
    }

    pub fn ready(system_id: impl Into<String>, observed_at_ms: u64) -> Self {
        Self {
            system_id: system_id.into(),
            state: SystemState::Ready,
            observed_at_ms,
            stale: false,
            invariant_failures: 0,
            queue_depth: 0,
            error_rate_ppm: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn is_tradable(&self) -> bool {
        matches!(self.state, SystemState::Ready) && !self.stale && self.invariant_failures == 0
    }

    pub fn is_fresh_at(&self, now_ms: u64, interval_ms: u64) -> bool {
        !self.stale
            && self.observed_at_ms <= now_ms
            && interval_ms > 0
            && now_ms.saturating_sub(self.observed_at_ms) <= interval_ms
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateSystem(String),
    MissingDependency { system: String, dependency: String },
    DependencyCycle,
    ImmutableReplacement(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateSystem(id) => write!(f, "duplicate system id: {id}"),
            Self::MissingDependency { system, dependency } => {
                write!(f, "system {system} depends on missing system {dependency}")
            }
            Self::DependencyCycle => write!(f, "system dependency cycle detected"),
            Self::ImmutableReplacement(id) => {
                write!(f, "immutable core system cannot be replaced: {id}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Why a registered system cannot currently participate in live execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessReport {
    pub system_id: String,
    pub checked_at_ms: u64,
    pub ready: bool,
    pub blockers: Vec<String>,
}

impl ReadinessReport {
    pub fn blocked(
        system_id: impl Into<String>,
        checked_at_ms: u64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            system_id: system_id.into(),
            checked_at_ms,
            ready: false,
            blockers: vec![reason.into()],
        }
    }
}

/// Runtime system topology and health registry.
///
/// The registry is the single topology source for supervisors, diagnostics,
/// dashboards and automated recovery. It does not execute strategy logic and
/// cannot mutate immutable exchange/order contracts.
#[derive(Debug, Clone)]
pub struct SystemRegistry {
    descriptors: BTreeMap<&'static str, SystemDescriptor>,
    health: BTreeMap<String, HealthSnapshot>,
}

impl Default for SystemRegistry {
    fn default() -> Self {
        Self::from_catalog(Self::catalog()).expect("built-in system catalog must be valid")
    }
}

impl SystemRegistry {
    pub fn catalog() -> Vec<SystemDescriptor> {
        vec![
            SystemDescriptor {
                id: "control.registry",
                layer: PlatformLayer::Control,
                role: SystemRole::Registry,
                authority: Authority::Internal,
                mutability: Mutability::ImmutableCore,
                dependencies: &[],
                health_interval_ms: 5_000,
                restartable: false,
            },
            SystemDescriptor {
                id: "market.binance",
                layer: PlatformLayer::MarketData,
                role: SystemRole::ExchangeAdapter,
                authority: Authority::Binance,
                mutability: Mutability::ImmutableCore,
                dependencies: &["control.registry"],
                health_interval_ms: 2_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "market.reference",
                layer: PlatformLayer::MarketData,
                role: SystemRole::ReferenceData,
                authority: Authority::ExternalReference,
                mutability: Mutability::ImmutableCore,
                dependencies: &["control.registry"],
                health_interval_ms: 30_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "market.anchor",
                layer: PlatformLayer::MarketData,
                role: SystemRole::Anchor,
                authority: Authority::Derived,
                mutability: Mutability::ImmutableCore,
                dependencies: &["market.binance", "market.reference"],
                health_interval_ms: 2_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "decision.strategy",
                layer: PlatformLayer::Decision,
                role: SystemRole::Strategy,
                authority: Authority::Internal,
                mutability: Mutability::GovernedPolicy,
                dependencies: &["market.anchor"],
                health_interval_ms: 1_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "decision.portfolio",
                layer: PlatformLayer::Decision,
                role: SystemRole::Portfolio,
                authority: Authority::Internal,
                mutability: Mutability::GovernedPolicy,
                dependencies: &["decision.strategy", "decision.risk"],
                health_interval_ms: 1_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "decision.risk",
                layer: PlatformLayer::Decision,
                role: SystemRole::Risk,
                authority: Authority::Internal,
                mutability: Mutability::ImmutableCore,
                dependencies: &["market.binance", "market.anchor"],
                health_interval_ms: 500,
                restartable: false,
            },
            SystemDescriptor {
                id: "decision.funding",
                layer: PlatformLayer::Decision,
                role: SystemRole::Funding,
                authority: Authority::Binance,
                mutability: Mutability::GovernedPolicy,
                dependencies: &["market.binance", "decision.risk"],
                health_interval_ms: 2_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "execution.gateway",
                layer: PlatformLayer::Execution,
                role: SystemRole::ExecutionGateway,
                authority: Authority::Binance,
                mutability: Mutability::ImmutableCore,
                dependencies: &["market.binance", "decision.risk"],
                health_interval_ms: 1_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "execution.lifecycle",
                layer: PlatformLayer::Execution,
                role: SystemRole::Lifecycle,
                authority: Authority::Binance,
                mutability: Mutability::ImmutableCore,
                dependencies: &["execution.gateway", "decision.risk"],
                health_interval_ms: 1_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "simulation.runtime",
                layer: PlatformLayer::Simulation,
                role: SystemRole::Simulation,
                authority: Authority::Derived,
                mutability: Mutability::GovernedPolicy,
                dependencies: &["market.anchor", "decision.strategy", "decision.risk"],
                health_interval_ms: 5_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "simulation.replay",
                layer: PlatformLayer::Simulation,
                role: SystemRole::Replay,
                authority: Authority::Derived,
                mutability: Mutability::GovernedPolicy,
                dependencies: &["market.binance"],
                health_interval_ms: 5_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "simulation.backtest",
                layer: PlatformLayer::Simulation,
                role: SystemRole::Backtest,
                authority: Authority::Derived,
                mutability: Mutability::GovernedPolicy,
                dependencies: &["simulation.replay", "decision.strategy"],
                health_interval_ms: 5_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "analytics.validation",
                layer: PlatformLayer::Analytics,
                role: SystemRole::Analytics,
                authority: Authority::Derived,
                mutability: Mutability::GovernedPolicy,
                dependencies: &["simulation.backtest", "observability.audit"],
                health_interval_ms: 10_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "observability.telemetry",
                layer: PlatformLayer::Observability,
                role: SystemRole::Observability,
                authority: Authority::Internal,
                mutability: Mutability::ImmutableCore,
                dependencies: &["control.registry"],
                health_interval_ms: 5_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "observability.audit",
                layer: PlatformLayer::Observability,
                role: SystemRole::Audit,
                authority: Authority::Internal,
                mutability: Mutability::ImmutableCore,
                dependencies: &["observability.telemetry", "execution.lifecycle"],
                health_interval_ms: 5_000,
                restartable: true,
            },
            SystemDescriptor {
                id: "control.recovery",
                layer: PlatformLayer::Control,
                role: SystemRole::Recovery,
                authority: Authority::Internal,
                mutability: Mutability::ImmutableCore,
                dependencies: &["control.registry", "execution.lifecycle"],
                health_interval_ms: 1_000,
                restartable: false,
            },
            SystemDescriptor {
                id: "control.console",
                layer: PlatformLayer::Control,
                role: SystemRole::ControlConsole,
                authority: Authority::Operator,
                mutability: Mutability::GovernedPolicy,
                dependencies: &["control.registry", "observability.telemetry"],
                health_interval_ms: 10_000,
                restartable: true,
            },
        ]
    }

    pub fn from_catalog(descriptors: Vec<SystemDescriptor>) -> Result<Self, RegistryError> {
        let mut registry = Self {
            descriptors: BTreeMap::new(),
            health: BTreeMap::new(),
        };
        for descriptor in descriptors {
            if registry.descriptors.contains_key(descriptor.id) {
                return Err(RegistryError::DuplicateSystem(descriptor.id.to_owned()));
            }
            registry.descriptors.insert(descriptor.id, descriptor);
        }
        registry.validate_topology()?;
        Ok(registry)
    }

    pub fn descriptors(&self) -> impl Iterator<Item = &SystemDescriptor> {
        self.descriptors.values()
    }

    pub fn descriptor(&self, id: &str) -> Option<&SystemDescriptor> {
        self.descriptors.get(id)
    }

    pub fn contract(&self, id: &str) -> Option<SystemContract> {
        self.descriptor(id).map(SystemDescriptor::contract)
    }

    pub fn contracts(&self) -> impl Iterator<Item = SystemContract> + '_ {
        self.descriptors.values().map(SystemDescriptor::contract)
    }

    pub fn health(&self, id: &str) -> Option<&HealthSnapshot> {
        self.health.get(id)
    }

    /// Register every discovered system before runtime tasks start. Missing or
    /// stale health is intentionally non-tradable until a producer reports ready.
    pub fn bootstrap_health(&mut self, observed_at_ms: u64) {
        for id in self.descriptors.keys() {
            self.health
                .entry((*id).to_owned())
                .or_insert_with(|| HealthSnapshot::discovered(*id, observed_at_ms));
        }
    }

    /// Convert overdue health reports into an explicit stale state.
    pub fn mark_stale_at(&mut self, now_ms: u64) -> Vec<String> {
        let mut changed = Vec::new();
        for (id, descriptor) in &self.descriptors {
            if let Some(snapshot) = self.health.get_mut(*id) {
                let stale = !snapshot.is_fresh_at(now_ms, descriptor.health_interval_ms);
                if stale != snapshot.stale {
                    snapshot.stale = stale;
                    if stale {
                        snapshot
                            .diagnostics
                            .push("health_report_expired".to_owned());
                    }
                    changed.push((*id).to_owned());
                }
            }
        }
        changed
    }

    /// Evaluate a system together with its complete dependency closure.
    pub fn readiness_at(&self, id: &str, now_ms: u64) -> ReadinessReport {
        let mut visiting = BTreeSet::new();
        self.readiness_visit(id, now_ms, &mut visiting)
    }

    pub fn require_ready(&self, id: &str, now_ms: u64) -> Result<(), ReadinessReport> {
        let report = self.readiness_at(id, now_ms);
        if report.ready {
            Ok(())
        } else {
            Err(report)
        }
    }

    fn readiness_visit(
        &self,
        id: &str,
        now_ms: u64,
        visiting: &mut BTreeSet<String>,
    ) -> ReadinessReport {
        if !visiting.insert(id.to_owned()) {
            return ReadinessReport::blocked(id, now_ms, "dependency_cycle");
        }
        let Some(descriptor) = self.descriptors.get(id) else {
            visiting.remove(id);
            return ReadinessReport::blocked(id, now_ms, "system_not_registered");
        };
        let mut blockers = Vec::new();
        match self.health.get(id) {
            None => blockers.push("health_report_missing".to_owned()),
            Some(snapshot) if !snapshot.is_tradable() => {
                blockers.push("health_not_tradable".to_owned())
            }
            Some(snapshot) if !snapshot.is_fresh_at(now_ms, descriptor.health_interval_ms) => {
                blockers.push("health_report_stale".to_owned())
            }
            Some(_) => {}
        }
        for dependency in descriptor.dependencies {
            let report = self.readiness_visit(dependency, now_ms, visiting);
            if !report.ready {
                blockers.extend(
                    report
                        .blockers
                        .into_iter()
                        .map(|reason| format!("{dependency}:{reason}")),
                );
            }
        }
        visiting.remove(id);
        ReadinessReport {
            system_id: id.to_owned(),
            checked_at_ms: now_ms,
            ready: blockers.is_empty(),
            blockers,
        }
    }

    pub fn report_health(&mut self, snapshot: HealthSnapshot) -> Result<(), RegistryError> {
        if !self.descriptors.contains_key(snapshot.system_id.as_str()) {
            return Err(RegistryError::MissingDependency {
                system: snapshot.system_id,
                dependency: "registered descriptor".to_owned(),
            });
        }
        self.health.insert(snapshot.system_id.clone(), snapshot);
        Ok(())
    }

    pub fn unhealthy(&self) -> impl Iterator<Item = &HealthSnapshot> {
        self.health
            .values()
            .filter(|snapshot| !snapshot.is_tradable())
    }

    pub fn replace_policy(
        &mut self,
        id: &str,
        replacement: SystemDescriptor,
    ) -> Result<(), RegistryError> {
        let current = self
            .descriptors
            .get(id)
            .ok_or_else(|| RegistryError::MissingDependency {
                system: id.to_owned(),
                dependency: "existing descriptor".to_owned(),
            })?;
        let current_id = current.id;
        if current.mutability == Mutability::ImmutableCore {
            return Err(RegistryError::ImmutableReplacement(id.to_owned()));
        }
        if replacement.id != id {
            return Err(RegistryError::DuplicateSystem(replacement.id.to_owned()));
        }
        let previous = self.descriptors.insert(replacement.id, replacement);
        if let Err(error) = self.validate_topology() {
            if let Some(previous) = previous {
                self.descriptors.insert(current_id, previous);
            }
            return Err(error);
        }
        Ok(())
    }

    pub fn validate_topology(&self) -> Result<(), RegistryError> {
        for descriptor in self.descriptors.values() {
            for dependency in descriptor.dependencies {
                if !self.descriptors.contains_key(dependency) {
                    return Err(RegistryError::MissingDependency {
                        system: descriptor.id.to_owned(),
                        dependency: (*dependency).to_owned(),
                    });
                }
            }
        }
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for id in self.descriptors.keys() {
            self.visit(id, &mut visiting, &mut visited)?;
        }
        Ok(())
    }

    fn visit(
        &self,
        id: &str,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> Result<(), RegistryError> {
        if visited.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id.to_owned()) {
            return Err(RegistryError::DependencyCycle);
        }
        let descriptor = self
            .descriptors
            .get(id)
            .expect("dependencies checked before cycle walk");
        for dependency in descriptor.dependencies {
            self.visit(dependency, visiting, visited)?;
        }
        visiting.remove(id);
        visited.insert(id.to_owned());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_catalog_is_complete_and_acyclic() {
        let registry = SystemRegistry::default();
        assert!(registry.descriptor("execution.gateway").is_some());
        assert!(registry.descriptor("simulation.runtime").is_some());
        assert!(registry.validate_topology().is_ok());
    }

    #[test]
    fn unknown_health_cannot_be_admitted() {
        let mut registry = SystemRegistry::default();
        let result = registry.report_health(HealthSnapshot::ready("unknown", 1));
        assert!(result.is_err());
    }

    #[test]
    fn immutable_core_cannot_be_replaced() {
        let mut registry = SystemRegistry::default();
        let replacement = SystemDescriptor {
            id: "decision.risk",
            layer: PlatformLayer::Decision,
            role: SystemRole::Risk,
            authority: Authority::Internal,
            mutability: Mutability::GovernedPolicy,
            dependencies: &["market.binance", "market.anchor"],
            health_interval_ms: 500,
            restartable: false,
        };
        assert!(matches!(
            registry.replace_policy("decision.risk", replacement),
            Err(RegistryError::ImmutableReplacement(_))
        ));
    }

    #[test]
    fn health_is_fail_closed() {
        let snapshot = HealthSnapshot::ready("decision.risk", 1);
        assert!(snapshot.is_tradable());
        let mut stale = snapshot.clone();
        stale.stale = true;
        assert!(!stale.is_tradable());
    }

    #[test]
    fn readiness_requires_health_for_the_complete_dependency_closure() {
        let mut registry = SystemRegistry::default();
        let blocked = registry.readiness_at("execution.gateway", 1_000);
        assert!(!blocked.ready);
        assert!(blocked
            .blockers
            .iter()
            .any(|reason| reason.contains("health_report_missing")));

        for id in [
            "control.registry",
            "market.binance",
            "market.reference",
            "market.anchor",
            "decision.risk",
            "execution.gateway",
        ] {
            registry
                .report_health(HealthSnapshot::ready(id, 1_000))
                .unwrap();
        }
        assert!(registry.readiness_at("execution.gateway", 1_000).ready);

        let changed = registry.mark_stale_at(2_001);
        assert!(changed.iter().any(|id| id == "decision.risk"));
        assert!(!registry.readiness_at("execution.gateway", 2_001).ready);
    }

    #[test]
    fn operational_systems_are_registered_as_first_class_nodes() {
        let registry = SystemRegistry::default();
        assert!(registry.descriptor("control.recovery").is_some());
        assert!(registry.descriptor("analytics.validation").is_some());
        assert!(registry.validate_topology().is_ok());
    }

    #[test]
    fn contracts_expose_capabilities_and_recovery() {
        let registry = SystemRegistry::default();
        let contract = registry.contract("execution.gateway").unwrap();
        assert_eq!(contract.requires, &["market.binance", "decision.risk"]);
        assert!(contract.provides.contains(&"execution.submit"));
        assert_eq!(contract.recovery, RecoveryPolicy::RestartThenReconcile);
        assert_eq!(registry.contracts().count(), registry.descriptors().count());
    }

    #[test]
    fn invalid_policy_replacement_is_atomic() {
        let mut registry = SystemRegistry::default();
        let replacement = SystemDescriptor {
            id: "decision.strategy",
            layer: PlatformLayer::Decision,
            role: SystemRole::Strategy,
            authority: Authority::Internal,
            mutability: Mutability::GovernedPolicy,
            dependencies: &["missing.parent"],
            health_interval_ms: 1_000,
            restartable: true,
        };
        assert!(registry
            .replace_policy("decision.strategy", replacement)
            .is_err());
        assert_eq!(
            registry
                .descriptor("decision.strategy")
                .expect("original descriptor remains")
                .dependencies,
            ["market.anchor"]
        );
    }
}
