# Sagebrush Constitution

## Core Principles

### I. Focused Unix Simplicity

Sagebrush MUST remain a focused CLI tool with a TUI. Every feature MUST address a stated user
need within the tool's purpose; unrelated capabilities MUST remain outside its scope. Designs
MUST use the simplest approach that satisfies current requirements and MUST NOT introduce
speculative abstractions or dependencies. CLI commands MUST compose with standard shell tools
through conventional streams and exit statuses rather than require interactive navigation.
This keeps the tool understandable, maintainable, and useful as part of a larger workflow.

### II. Help Is the Complete User Guide

Built-in help MUST provide all information needed to use every supported user-facing capability
without consulting external documentation. Root help MUST explain the tool's purpose and expose
commands and TUI entry points. Command help MUST explain arguments, options, defaults, relevant
configuration, and representative examples. TUI help MUST expose navigation, key bindings,
available actions, and how to exit. Configuration precedence, file discovery, and recovery from
common errors MUST be documented in help reachable from the root help. External documentation
MAY supplement this material but MUST NOT be the only source of required usage information.
Help updates MUST accompany the behavior they describe.

### III. Intuitive and Enjoyable Terminal Experience

CLI and TUI terminology, defaults, and outcomes MUST agree for equivalent operations. Commands
and screens MUST make available actions discoverable and provide clear success, progress,
empty-state, and error feedback where applicable. Errors MUST explain the failure and give an
actionable recovery step when one is known. TUI navigation, help, cancellation, and exit MUST be
keyboard-accessible. Destructive operations MUST require explicit intent through confirmation
or an explicit non-interactive option. Personality and playful touches MAY make the tool fun,
but MUST NOT obscure meaning, slow required workflows, or contaminate script-consumed output.
Enjoyment comes from clarity and control, not surprise.

### IV. Test-First Confidence

Behavior changes SHOULD follow a red-green-refactor cycle: write a focused test, observe it
fail for the intended reason, implement the smallest passing change, then refactor. This is the
default because executable examples clarify intent before implementation. When test-first work
is impractical, the change MUST record the reason and alternative verification in its review
or implementation notes. New or changed behavior MUST have automated coverage where feasible;
any coverage exception MUST identify the limitation and reproducible manual checks. Bug fixes
MUST include a regression test when reproducible in the test environment. Tests MUST exercise
observable behavior rather than incidental implementation details.

### V. Opinionated Defaults, Explicit Adaptability

The default configuration MUST support the primary documented workflow without requiring users
to choose implementation details. Each new configuration option MUST address a concrete user
need rather than substitute for a product decision. Supported configuration MUST have documented
defaults, accepted values, discovery rules, and deterministic precedence. Invalid values MUST
produce actionable errors rather than silently fall back. Equivalent CLI and TUI operations
MUST resolve shared configuration consistently. Changes to defaults or configuration contracts
MUST document user impact and any required migration. Adaptability MUST preserve a simple
out-of-the-box experience.

## Terminal Interface Requirements

- Scriptable operations MUST offer a non-interactive CLI path; the TUI MUST NOT be required for
  automation.
- CLI result data MUST go to stdout; diagnostics MUST go to stderr. Successful commands MUST
  return zero and failed commands MUST return a nonzero exit status.
- Non-interactive execution MUST NOT wait for hidden prompts. Missing required input MUST
  produce an actionable error.
- Presentation MUST respect terminal capabilities. Color MUST NOT be the sole carrier of
  meaning, and redirected script output MUST NOT contain TUI control sequences.
- The TUI MUST restore terminal state on normal exit, cancellation, and handled errors.

## Development Workflow and Quality Gates

Before implementation, each feature or behavior change MUST define its user outcome, scope,
acceptance criteria, help impact, and configuration impact. Plans MUST check the five core
principles and identify any proposed exception before implementation.

Implementation MUST keep changes focused and follow the test-first default. Review MUST verify
the changed CLI, TUI, help, and configuration contracts as applicable. Relevant automated tests
and existing build, type, and lint checks MUST pass before merge; unavailable checks MUST be
explicitly reported rather than treated as passing.

For changed user workflows, reviewers MUST verify that a user can complete the acceptance
scenario using built-in help alone. CLI stream and exit behavior, TUI navigation and recovery,
and configuration defaults and overrides MUST be checked when affected. Manual checks MUST
record reproducible steps and outcomes.

## Governance

This constitution is the authoritative source of Sagebrush project principles. Feature
specifications, plans, implementation tasks, and reviews MUST conform to it. Conflicts MUST be
resolved through a compliant design or an approved constitutional amendment.

Amendments MUST state the proposed change, rationale, compatibility impact, and any migration or
follow-up work. A project maintainer MUST approve an amendment before adoption. Each amendment
MUST update the version and last-amended date while preserving the original ratification date.
Constitution versions use semantic versioning: MAJOR for incompatible principle or governance
removals or redefinitions, MINOR for new principles or materially expanded guidance, and PATCH
for non-semantic clarifications or corrections.

Every implementation review MUST check constitutional compliance. Exceptions MUST identify the
affected rule, rationale, scope, compensating measures, and maintainer approval. An exception
MUST NOT silently redefine a principle; recurring or permanent departures require an amendment.
Dependent guidance MUST be assessed for consistency when an amendment is adopted, with required
updates tracked as follow-up work rather than silently changing its meaning.

**Version**: 1.0.0 | **Ratified**: 2026-10-21 | **Last Amended**: 2026-10-21
