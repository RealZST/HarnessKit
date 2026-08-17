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
    CredentialTheft, DangerousCommands, DshJsEnvNoFallback, PlaintextSecrets, PromptInjection,
    RemoteCodeExecution, SafetyBypass, SkillInvocationKeyCase,
};
/// Scanner-only: dsh drops camelCase-invocation-key skills wholesale, so the
/// scanner must not emit them for dsh. Same key vocabulary as the
/// `skill-invocation-key-case` rule, asked as a yes/no question.
pub(crate) use content::dsh_drops_skill_for_invocation_key;
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
        Box::new(DshJsEnvNoFallback),
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
