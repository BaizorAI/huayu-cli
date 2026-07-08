//! Built-in skills embedded at compile time via `include_str!`.
//!
//! These are installed on first launch into `~/.huayu/claude/skills/`
//! (Claude Code) and `~/.huayu/codex/rules.md` (Codex).

/// Built-in Claude Code skills: (filename, content) pairs.
pub const BUILTIN_CLAUDE_SKILLS: &[(&str, &str)] = &[
    ("code-review.md", include_str!("../skills/claude/code-review.md")),
    ("refactor.md", include_str!("../skills/claude/refactor.md")),
];

/// Built-in Codex rules file content.
pub const BUILTIN_CODEX_RULES: &str = include_str!("../skills/codex/rules.md");
