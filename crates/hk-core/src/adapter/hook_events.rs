/// Centralized hook event name mapping between agents.
///
/// Each agent uses its own event naming convention:
/// - Claude/Codex: PascalCase (Stop, PreToolUse, PostToolUse, ...)
/// - Gemini: PascalCase but different names (AfterAgent, BeforeTool, AfterTool, ...)
/// - Cursor: camelCase (stop, preToolUse, postToolUse, sessionStart, ...)
/// - Copilot: camelCase (sessionEnd, preToolUse, postToolUse, ...)
/// - Antigravity: does not support hooks (use rules/workflows instead)
///
/// This module provides a canonical intermediate form (Claude's names) and
/// per-agent translation functions.
///
/// Official hook documentation:
/// - Claude Code: https://code.claude.com/docs/en/hooks
/// - Codex CLI:   https://developers.openai.com/codex/hooks
/// - Cursor:      https://cursor.com/docs/hooks
/// - Gemini CLI:  https://geminicli.com/docs/hooks/
/// - Copilot:     https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/use-hooks
/// - Antigravity: no hook support — use rules instead: https://antigravity.google/docs/rules-workflows

/// Canonical event names (Claude's convention) used as the internal lingua franca.
const CANONICAL_EVENTS: &[&str] = &[
    "Stop",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "UserPromptSubmit",
    "SessionStart",
    "SessionEnd",
    "Notification",
    "PreCompact",
    "PostCompact",
    "SubagentStart",
    "SubagentStop",
    "PermissionRequest",
];

/// A single mapping entry: (canonical_name, agent_name)
struct EventMapping {
    canonical: &'static str,
    agent: &'static str,
}

/// Claude/Codex event mappings (identity — they use canonical names)
const CLAUDE_EVENTS: &[EventMapping] = &[
    EventMapping { canonical: "Stop", agent: "Stop" },
    EventMapping { canonical: "PreToolUse", agent: "PreToolUse" },
    EventMapping { canonical: "PostToolUse", agent: "PostToolUse" },
    EventMapping { canonical: "PostToolUseFailure", agent: "PostToolUseFailure" },
    EventMapping { canonical: "UserPromptSubmit", agent: "UserPromptSubmit" },
    EventMapping { canonical: "SessionStart", agent: "SessionStart" },
    EventMapping { canonical: "SessionEnd", agent: "SessionEnd" },
    EventMapping { canonical: "Notification", agent: "Notification" },
    EventMapping { canonical: "PreCompact", agent: "PreCompact" },
    EventMapping { canonical: "PostCompact", agent: "PostCompact" },
    EventMapping { canonical: "SubagentStart", agent: "SubagentStart" },
    EventMapping { canonical: "SubagentStop", agent: "SubagentStop" },
    EventMapping { canonical: "PermissionRequest", agent: "PermissionRequest" },
];

/// Gemini event mappings
const GEMINI_EVENTS: &[EventMapping] = &[
    EventMapping { canonical: "Stop", agent: "AfterAgent" },
    EventMapping { canonical: "PreToolUse", agent: "BeforeTool" },
    EventMapping { canonical: "PostToolUse", agent: "AfterTool" },
    EventMapping { canonical: "UserPromptSubmit", agent: "BeforeAgent" },
    EventMapping { canonical: "SessionStart", agent: "SessionStart" },
    EventMapping { canonical: "SessionEnd", agent: "SessionEnd" },
    EventMapping { canonical: "Notification", agent: "Notification" },
    EventMapping { canonical: "PreCompact", agent: "PreCompress" },
];

/// Cursor event mappings
/// Since v2.4, Cursor supports native preToolUse/postToolUse and many more events.
/// See: https://cursor.com/docs/hooks
const CURSOR_EVENTS: &[EventMapping] = &[
    EventMapping { canonical: "Stop", agent: "stop" },
    EventMapping { canonical: "PreToolUse", agent: "preToolUse" },
    EventMapping { canonical: "PostToolUse", agent: "postToolUse" },
    EventMapping { canonical: "PostToolUseFailure", agent: "postToolUseFailure" },
    EventMapping { canonical: "UserPromptSubmit", agent: "beforeSubmitPrompt" },
    EventMapping { canonical: "SessionStart", agent: "sessionStart" },
    EventMapping { canonical: "SessionEnd", agent: "sessionEnd" },
    EventMapping { canonical: "PreCompact", agent: "preCompact" },
    EventMapping { canonical: "SubagentStart", agent: "subagentStart" },
    EventMapping { canonical: "SubagentStop", agent: "subagentStop" },
];

/// Copilot event mappings
const COPILOT_EVENTS: &[EventMapping] = &[
    EventMapping { canonical: "Stop", agent: "sessionEnd" },
    EventMapping { canonical: "PreToolUse", agent: "preToolUse" },
    EventMapping { canonical: "PostToolUse", agent: "postToolUse" },
    EventMapping { canonical: "UserPromptSubmit", agent: "userPromptSubmitted" },
    EventMapping { canonical: "SessionStart", agent: "sessionStart" },
    EventMapping { canonical: "SessionEnd", agent: "sessionEnd" },
];

/// Translate an event name from any agent's convention to the target agent's convention.
/// Returns None if the event has no equivalent in the target agent.
fn translate(event: &str, from_table: &[EventMapping], to_table: &[EventMapping]) -> Option<String> {
    // First check if it's already a native name in the target table
    if to_table.iter().any(|m| m.agent == event) {
        return Some(event.to_string());
    }
    // Find canonical form from source table
    let canonical = from_table.iter()
        .find(|m| m.agent == event)
        .map(|m| m.canonical)
        // Also check if the event is already in canonical form
        .or_else(|| CANONICAL_EVENTS.iter().find(|&&c| c == event).copied())?;
    // Map canonical to target
    to_table.iter()
        .find(|m| m.canonical == canonical)
        .map(|m| m.agent.to_string())
}

/// Translate an event name to Claude/Codex convention.
pub fn to_claude(event: &str) -> Option<String> {
    // Try all source tables
    translate(event, CLAUDE_EVENTS, CLAUDE_EVENTS)
        .or_else(|| translate(event, GEMINI_EVENTS, CLAUDE_EVENTS))
        .or_else(|| translate(event, CURSOR_EVENTS, CLAUDE_EVENTS))
        .or_else(|| translate(event, COPILOT_EVENTS, CLAUDE_EVENTS))
}

/// Translate an event name to Gemini convention.
pub fn to_gemini(event: &str) -> Option<String> {
    translate(event, GEMINI_EVENTS, GEMINI_EVENTS)
        .or_else(|| translate(event, CLAUDE_EVENTS, GEMINI_EVENTS))
        .or_else(|| translate(event, CURSOR_EVENTS, GEMINI_EVENTS))
        .or_else(|| translate(event, COPILOT_EVENTS, GEMINI_EVENTS))
}

/// Translate an event name to Cursor convention.
pub fn to_cursor(event: &str) -> Option<String> {
    translate(event, CURSOR_EVENTS, CURSOR_EVENTS)
        .or_else(|| translate(event, CLAUDE_EVENTS, CURSOR_EVENTS))
        .or_else(|| translate(event, GEMINI_EVENTS, CURSOR_EVENTS))
        .or_else(|| translate(event, COPILOT_EVENTS, CURSOR_EVENTS))
}

/// Translate an event name to Copilot convention.
pub fn to_copilot(event: &str) -> Option<String> {
    translate(event, COPILOT_EVENTS, COPILOT_EVENTS)
        .or_else(|| translate(event, CLAUDE_EVENTS, COPILOT_EVENTS))
        .or_else(|| translate(event, GEMINI_EVENTS, COPILOT_EVENTS))
        .or_else(|| translate(event, CURSOR_EVENTS, COPILOT_EVENTS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_to_gemini() {
        assert_eq!(to_gemini("Stop"), Some("AfterAgent".into()));
        assert_eq!(to_gemini("PreToolUse"), Some("BeforeTool".into()));
    }

    #[test]
    fn gemini_to_claude() {
        assert_eq!(to_claude("AfterAgent"), Some("Stop".into()));
        assert_eq!(to_claude("BeforeTool"), Some("PreToolUse".into()));
    }

    #[test]
    fn cursor_to_gemini() {
        assert_eq!(to_gemini("stop"), Some("AfterAgent".into()));
        assert_eq!(to_gemini("preToolUse"), Some("BeforeTool".into()));
        assert_eq!(to_gemini("postToolUse"), Some("AfterTool".into()));
    }

    #[test]
    fn copilot_to_cursor() {
        assert_eq!(to_cursor("preToolUse"), Some("preToolUse".into()));
        assert_eq!(to_cursor("sessionEnd"), Some("sessionEnd".into()));
    }

    #[test]
    fn passthrough_native() {
        assert_eq!(to_claude("Stop"), Some("Stop".into()));
        assert_eq!(to_gemini("AfterAgent"), Some("AfterAgent".into()));
        assert_eq!(to_cursor("stop"), Some("stop".into()));
        assert_eq!(to_copilot("preToolUse"), Some("preToolUse".into()));
    }

    #[test]
    fn unsupported_event() {
        assert_eq!(to_cursor("Notification"), None);
        assert_eq!(to_copilot("PreCompact"), None);
        assert_eq!(to_gemini("PermissionRequest"), None);
    }
}
