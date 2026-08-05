pub mod catalog;
pub mod doctor;
pub mod evidence;
pub mod policy;
pub mod runtime;
pub mod workload;

pub use catalog::{Catalog, Lab, Project, RuntimeDescriptor};
pub use doctor::{DoctorCheck, DoctorReport};
pub use evidence::{Evidence, EvidenceStatus, Violation};
pub use policy::{EnforcementMode, Policy, ResourcePolicy};
pub use runtime::{command_exists, ControlAssessment, ExecutionOutcome, ExecutionPlan, RuntimeKind, RuntimeProbe};
pub use workload::{ExpectedOutcome, Workload};
