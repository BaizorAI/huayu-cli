---
name: code-review
description: Review code changes for correctness, bugs, simplification, and efficiency. Use before committing nontrivial changes.
---

# Code Review

Review the current diff or specified code for:

1. **Correctness bugs** — logic errors, edge cases, null/undefined handling, off-by-one errors
2. **Simplification** — redundant code, over-complicated patterns, dead code
3. **Efficiency** — unnecessary allocations, N+1 queries, blocking operations in async contexts
4. **Reuse** — opportunities to use existing utilities, shared components, or library functions

## Instructions

- Start by reading the changed files to understand the scope
- Verify that all modified functions handle edge cases (empty inputs, null values, error states)
- Check that new code follows the project's existing patterns and conventions
- Report findings ranked by severity: critical → minor → suggestion
- For each finding, include the file path, line reference, and a suggested fix
