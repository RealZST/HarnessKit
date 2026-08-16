use crate::auditor::AuditRule;

mod cli;
mod content;
mod mcp;
mod permissions;
mod plugin;
mod shared;
#[cfg(test)]
mod test_support;

pub use cli::{
    CliAggregateRisk, CliBinarySource, CliCredentialStorage, CliNetworkAccess, CliPermissionScope,
};
pub use content::{
    CredentialTheft, DangerousCommands, PlaintextSecrets, PromptInjection, RemoteCodeExecution,
    SafetyBypass, SkillInvocationKeyCase,
};
pub use mcp::McpCommandInjection;
pub use permissions::{
    BroadPermissions, PermissionCombinationRisk, SupplyChainRisk, UnknownSource,
};
pub use plugin::{PluginLifecycleScripts, PluginSourceTrust};

pub fn all_rules() -> Vec<Box<dyn AuditRule>> {
    vec![
        Box::new(PromptInjection),
        Box::new(RemoteCodeExecution),
        Box::new(CredentialTheft),
        Box::new(PlaintextSecrets),
        Box::new(SafetyBypass),
        Box::new(DangerousCommands),
        Box::new(SkillInvocationKeyCase),
        Box::new(BroadPermissions),
        Box::new(SupplyChainRisk),
        Box::new(UnknownSource),
        Box::new(PermissionCombinationRisk),
        Box::new(CliCredentialStorage),
        Box::new(CliNetworkAccess),
        Box::new(CliBinarySource),
        Box::new(CliPermissionScope),
        Box::new(CliAggregateRisk),
        Box::new(McpCommandInjection),
        Box::new(PluginSourceTrust),
        Box::new(PluginLifecycleScripts),
    ]
}
