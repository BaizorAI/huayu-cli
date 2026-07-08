---
name: refactor
description: Guide for safe, incremental refactoring. Use when asked to refactor, restructure, or improve code organization.
---

# Refactoring

Apply safe, incremental refactoring following these principles:

1. **Small steps** — each change should be independently testable and reversible
2. **Preserve behavior** — the refactor must not change any external behavior
3. **Tests first** — ensure tests pass before and after each step
4. **Extract, don't rewrite** — prefer extracting functions/modules over rewriting from scratch

## Common Refactoring Patterns

- **Extract Function**: Move a block of code into a named function
- **Extract Module**: Move related functions/types into a separate file
- **Replace Magic Numbers**: Introduce named constants
- **Simplify Conditionals**: Replace nested if-else with early returns or match
- **Introduce Type Alias**: Simplify complex type signatures

## Safety Checklist

- [ ] All existing tests pass before starting
- [ ] Each intermediate step compiles successfully
- [ ] No public API surface changed (unless explicitly requested)
- [ ] All existing tests pass after each step
