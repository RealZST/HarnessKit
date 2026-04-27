//! Centralized hook event name mapping between agents.
//!
//! Each agent uses its own event naming convention:
//! - Claude/Codex: PascalCase (Stop, PreToolUse, PostToolUse, ...)
//! - Gemini: PascalCase but different names (AfterAgent, BeforeTool, AfterTool, ...)
//! - Cursor: camelCase (stop, preToolUse, postToolUse, sessionStart, ...)
//! - Copilot: camelCase (sessionEnd, preToolUse, postToolUse, ...)
//! - Windsurf: snake_case (pre_user_prompt, post_cascade_response, ...)
//! - Antigravity: does not support hooks (use rules/workflows instead)
//!
//! This module provides a canonical intermediate form (Claude's names) and
//! per-agent translation functions.
//!
//! Official hook documentation:
//! - Claude Code: https://code.claude.com/docs/en/hooks
//! - Codex CLI:   https://developers.openai.com/codex/hooks
//! - Cursor:      https://cursor.com/docs/hooks
//! - Gemini CLI:  https://geminicli.com/docs/hooks/
//! - Copilot:     https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/use-hooks
//! - Windsurf:    https://docs.windsurf.com/windsurf/cascade/hooks
//! - Antigravity: no hook support — use rules instead: https://antigravity.google/docs/rules-workflows

// Canonical event names (Claude's convention) used as the internal lingua franca.
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

/// A single mapping entry: (canonical_name, agent_name).
///
/// For events that have a true cross-agent equivalent, `canonical` is the
/// canonical (Claude) event name shared across tables — e.g. Cursor's
/// `preToolUse` and Windsurf's `pre_user_prompt` both have a canonical form.
///
/// For agent-specific events with no canonical equivalent (e.g. Windsurf's
/// `pre_run_command`, which is narrower than Claude's `PreToolUse`), the
/// convention is `canonical == agent`. This enables `to_<agent>(<own-event>)`
/// passthrough while keeping cross-agent translation correctly returning None
/// — relies on the invariant that no other agent table uses the same string
/// as a canonical or agent name. See `WINDSURF_EVENTS` for the rationale.
struct EventMapping {
    canonical: &'static str,
    agent: &'static str,
}

/// Claude/Codex event mappings (identity — they use canonical names)
const CLAUDE_EVENTS: &[EventMapping] = &[
    EventMapping {
        canonical: "Stop",
        agent: "Stop",
    },
    EventMapping {
        canonical: "PreToolUse",
        agent: "PreToolUse",
    },
    EventMapping {
        canonical: "PostToolUse",
        agent: "PostToolUse",
    },
    EventMapping {
        canonical: "PostToolUseFailure",
        agent: "PostToolUseFailure",
    },
    EventMapping {
        canonical: "UserPromptSubmit",
        agent: "UserPromptSubmit",
    },
    EventMapping {
        canonical: "SessionStart",
        agent: "SessionStart",
    },
    EventMapping {
        canonical: "SessionEnd",
        agent: "SessionEnd",
    },
    EventMapping {
        canonical: "Notification",
        agent: "Notification",
    },
    EventMapping {
        canonical: "PreCompact",
        agent: "PreCompact",
    },
    EventMapping {
        canonical: "PostCompact",
        agent: "PostCompact",
    },
    EventMapping {
        canonical: "SubagentStart",
        agent: "SubagentStart",
    },
    EventMapping {
        canonical: "SubagentStop",
        agent: "SubagentStop",
    },
    EventMapping {
        canonical: "PermissionRequest",
        agent: "PermissionRequest",
    },
];

/// Gemini event mappings
const GEMINI_EVENTS: &[EventMapping] = &[
    EventMapping {
        canonical: "Stop",
        agent: "AfterAgent",
    },
    EventMapping {
        canonical: "PreToolUse",
        agent: "BeforeTool",
    },
    EventMapping {
        canonical: "PostToolUse",
        agent: "AfterTool",
    },
    EventMapping {
        canonical: "UserPromptSubmit",
        agent: "BeforeAgent",
    },
    EventMapping {
        canonical: "SessionStart",
        agent: "SessionStart",
    },
    EventMapping {
        canonical: "SessionEnd",
        agent: "SessionEnd",
    },
    EventMapping {
        canonical: "Notification",
        agent: "Notification",
    },
    EventMapping {
        canonical: "PreCompact",
        agent: "PreCompress",
    },
];

/// Cursor event mappings
/// Since v2.4, Cursor supports native preToolUse/postToolUse and many more events.
/// See: https://cursor.com/docs/hooks
const CURSOR_EVENTS: &[EventMapping] = &[
    EventMapping {
        canonical: "Stop",
        agent: "stop",
    },
    EventMapping {
        canonical: "PreToolUse",
        agent: "preToolUse",
    },
    EventMapping {
        canonical: "PostToolUse",
        agent: "postToolUse",
    },
    EventMapping {
        canonical: "PostToolUseFailure",
        agent: "postToolUseFailure",
    },
    EventMapping {
        canonical: "UserPromptSubmit",
        agent: "beforeSubmitPrompt",
    },
    EventMapping {
        canonical: "SessionStart",
        agent: "sessionStart",
    },
    EventMapping {
        canonical: "SessionEnd",
        agent: "sessionEnd",
    },
    EventMapping {
        canonical: "PreCompact",
        agent: "preCompact",
    },
    EventMapping {
        canonical: "SubagentStart",
        agent: "subagentStart",
    },
    EventMapping {
        canonical: "SubagentStop",
        agent: "subagentStop",
    },
];

/// Copilot event mappings (VS Code / Copilot CLI use PascalCase, same as Claude)
/// Reference: https://code.visualstudio.com/docs/copilot/customization/hooks
const COPILOT_EVENTS: &[EventMapping] = &[
    EventMapping {
        canonical: "Stop",
        agent: "Stop",
    },
    EventMapping {
        canonical: "PreToolUse",
        agent: "PreToolUse",
    },
    EventMapping {
        canonical: "PostToolUse",
        agent: "PostToolUse",
    },
    EventMapping {
        canonical: "UserPromptSubmit",
        agent: "UserPromptSubmit",
    },
    EventMapping {
        canonical: "SessionStart",
        agent: "SessionStart",
    },
    EventMapping {
        canonical: "SubagentStart",
        agent: "SubagentStart",
    },
    EventMapping {
        canonical: "SubagentStop",
        agent: "SubagentStop",
    },
];

/// Windsurf event mappings.
///
/// Windsurf provides 12 hook events; only 2 have a strict functional equivalent
/// in the canonical (Claude) taxonomy. The other 10 are listed with `canonical`
/// equal to `agent`, which makes `to_windsurf(<windsurf-event>)` work as a
/// passthrough while still returning `None` for cross-agent translation —
/// no Claude/Gemini/Cursor/Copilot event shares those canonical names.
///
/// Why so few mappings? Windsurf splits "tool use" into 4 narrow categories
/// (`read_code` / `write_code` / `run_command` / `mcp_tool_use`) while Claude's
/// `PreToolUse` covers all tools. Mapping any single Windsurf category to
/// `PreToolUse`/`PostToolUse` would silently change a hook's trigger set when
/// deployed across agents (over-fire in one direction, under-fire in the other),
/// breaking the "deploy a hook unchanged across agents" contract.
///
/// Reference: https://docs.windsurf.com/windsurf/cascade/hooks
const WINDSURF_EVENTS: &[EventMapping] = &[
    // --- Mapped (functionally equivalent to a canonical event) ---
    EventMapping {
        canonical: "UserPromptSubmit",
        agent: "pre_user_prompt",
    },
    EventMapping {
        canonical: "Stop",
        agent: "post_cascade_response",
    },
    // --- Windsurf-specific (no canonical equivalent; passthrough only) ---
    EventMapping {
        canonical: "pre_read_code",
        agent: "pre_read_code",
    },
    EventMapping {
        canonical: "post_read_code",
        agent: "post_read_code",
    },
    EventMapping {
        canonical: "pre_write_code",
        agent: "pre_write_code",
    },
    EventMapping {
        canonical: "post_write_code",
        agent: "post_write_code",
    },
    EventMapping {
        canonical: "pre_run_command",
        agent: "pre_run_command",
    },
    EventMapping {
        canonical: "post_run_command",
        agent: "post_run_command",
    },
    EventMapping {
        canonical: "pre_mcp_tool_use",
        agent: "pre_mcp_tool_use",
    },
    EventMapping {
        canonical: "post_mcp_tool_use",
        agent: "post_mcp_tool_use",
    },
    EventMapping {
        canonical: "post_cascade_response_with_transcript",
        agent: "post_cascade_response_with_transcript",
    },
    EventMapping {
        canonical: "post_setup_worktree",
        agent: "post_setup_worktree",
    },
];

/// Translate an event name from any agent's convention to the target agent's convention.
/// Returns None if the event has no equivalent in the target agent.
fn translate(
    event: &str,
    from_table: &[EventMapping],
    to_table: &[EventMapping],
) -> Option<String> {
    // First check if it's already a native name in the target table
    if to_table.iter().any(|m| m.agent == event) {
        return Some(event.to_string());
    }
    // Find canonical form from source table
    let canonical = from_table
        .iter()
        .find(|m| m.agent == event)
        .map(|m| m.canonical)
        // Also check if the event is already in canonical form
        .or_else(|| CANONICAL_EVENTS.iter().find(|&&c| c == event).copied())?;
    // Map canonical to target
    to_table
        .iter()
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
        .or_else(|| translate(event, WINDSURF_EVENTS, CLAUDE_EVENTS))
}

/// Translate an event name to Gemini convention.
pub fn to_gemini(event: &str) -> Option<String> {
    translate(event, GEMINI_EVENTS, GEMINI_EVENTS)
        .or_else(|| translate(event, CLAUDE_EVENTS, GEMINI_EVENTS))
        .or_else(|| translate(event, CURSOR_EVENTS, GEMINI_EVENTS))
        .or_else(|| translate(event, COPILOT_EVENTS, GEMINI_EVENTS))
        .or_else(|| translate(event, WINDSURF_EVENTS, GEMINI_EVENTS))
}

/// Translate an event name to Cursor convention.
pub fn to_cursor(event: &str) -> Option<String> {
    translate(event, CURSOR_EVENTS, CURSOR_EVENTS)
        .or_else(|| translate(event, CLAUDE_EVENTS, CURSOR_EVENTS))
        .or_else(|| translate(event, GEMINI_EVENTS, CURSOR_EVENTS))
        .or_else(|| translate(event, COPILOT_EVENTS, CURSOR_EVENTS))
        .or_else(|| translate(event, WINDSURF_EVENTS, CURSOR_EVENTS))
}

/// Translate an event name to Copilot convention.
pub fn to_copilot(event: &str) -> Option<String> {
    translate(event, COPILOT_EVENTS, COPILOT_EVENTS)
        .or_else(|| translate(event, CLAUDE_EVENTS, COPILOT_EVENTS))
        .or_else(|| translate(event, GEMINI_EVENTS, COPILOT_EVENTS))
        .or_else(|| translate(event, CURSOR_EVENTS, COPILOT_EVENTS))
        .or_else(|| translate(event, WINDSURF_EVENTS, COPILOT_EVENTS))
}

/// Translate an event name to Windsurf convention.
pub fn to_windsurf(event: &str) -> Option<String> {
    translate(event, WINDSURF_EVENTS, WINDSURF_EVENTS)
        .or_else(|| translate(event, CLAUDE_EVENTS, WINDSURF_EVENTS))
        .or_else(|| translate(event, GEMINI_EVENTS, WINDSURF_EVENTS))
        .or_else(|| translate(event, CURSOR_EVENTS, WINDSURF_EVENTS))
        .or_else(|| translate(event, COPILOT_EVENTS, WINDSURF_EVENTS))
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
        assert_eq!(to_cursor("PreToolUse"), Some("preToolUse".into()));
        assert_eq!(to_cursor("Stop"), Some("stop".into()));
    }

    #[test]
    fn windsurf_round_trips_supported_events() {
        assert_eq!(
            to_windsurf("UserPromptSubmit"),
            Some("pre_user_prompt".into())
        );
        assert_eq!(to_windsurf("Stop"), Some("post_cascade_response".into()));
        assert_eq!(
            to_claude("pre_user_prompt"),
            Some("UserPromptSubmit".into())
        );
        assert_eq!(to_claude("post_cascade_response"), Some("Stop".into()));
    }

    #[test]
    fn passthrough_native() {
        assert_eq!(to_claude("Stop"), Some("Stop".into()));
        assert_eq!(to_gemini("AfterAgent"), Some("AfterAgent".into()));
        assert_eq!(to_cursor("stop"), Some("stop".into()));
        assert_eq!(to_copilot("PreToolUse"), Some("PreToolUse".into()));
        assert_eq!(
            to_windsurf("pre_user_prompt"),
            Some("pre_user_prompt".into())
        );
    }

    #[test]
    fn unsupported_event() {
        assert_eq!(to_cursor("Notification"), None);
        assert_eq!(to_gemini("PermissionRequest"), None);
        assert_eq!(to_windsurf("PreToolUse"), None);
    }

    /// The 10 Windsurf-specific events have no canonical equivalent;
    /// they must passthrough when the target agent is Windsurf itself.
    #[test]
    fn windsurf_specific_events_passthrough_to_self() {
        let specific = [
            "pre_read_code",
            "post_read_code",
            "pre_write_code",
            "post_write_code",
            "pre_run_command",
            "post_run_command",
            "pre_mcp_tool_use",
            "post_mcp_tool_use",
            "post_cascade_response_with_transcript",
            "post_setup_worktree",
        ];
        for event in specific {
            assert_eq!(
                to_windsurf(event),
                Some(event.to_string()),
                "to_windsurf passthrough failed for '{}'",
                event
            );
        }
    }

    /// Windsurf-specific events have no semantic equivalent on other agents;
    /// translating them must return None to avoid silently changing trigger
    /// sets when a hook is deployed across agents.
    #[test]
    fn windsurf_specific_events_do_not_cross_translate() {
        let specific = [
            "pre_read_code",
            "post_read_code",
            "pre_write_code",
            "post_write_code",
            "pre_run_command",
            "post_run_command",
            "pre_mcp_tool_use",
            "post_mcp_tool_use",
            "post_cascade_response_with_transcript",
            "post_setup_worktree",
        ];
        for event in specific {
            assert_eq!(to_claude(event), None, "to_claude leaked for '{}'", event);
            assert_eq!(to_gemini(event), None, "to_gemini leaked for '{}'", event);
            assert_eq!(to_cursor(event), None, "to_cursor leaked for '{}'", event);
            assert_eq!(
                to_copilot(event),
                None,
                "to_copilot leaked for '{}'",
                event
            );
        }
    }

    /// Claude's `PreToolUse`/`PostToolUse` (and the equivalent events on Gemini,
    /// Cursor, Copilot) cover all tools, while Windsurf splits tool events into
    /// 4 narrow categories (read_code/write_code/run_command/mcp_tool_use).
    /// Mapping them would silently change the trigger set of a deployed hook
    /// (over-fire or under-fire), so the translation must return None for every
    /// agent's tool event.
    #[test]
    fn tool_events_from_any_agent_do_not_map_to_windsurf() {
        // Claude / Copilot (PascalCase)
        assert_eq!(to_windsurf("PreToolUse"), None);
        assert_eq!(to_windsurf("PostToolUse"), None);
        // Gemini (PascalCase, different names)
        assert_eq!(to_windsurf("BeforeTool"), None);
        assert_eq!(to_windsurf("AfterTool"), None);
        // Cursor (camelCase)
        assert_eq!(to_windsurf("preToolUse"), None);
        assert_eq!(to_windsurf("postToolUse"), None);
    }
}
