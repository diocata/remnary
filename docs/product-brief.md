# OBSERVR
## Universal AI Observability & Personal Intelligence Engine

> **Your conversations tell a story. Observr helps you understand it.**

**Status:** Concept / Planning  
**Version:** 0.1.0 (Proposed MVP)  
**Type:** Open-source, local-first application  
**Platforms:** macOS, Linux, Windows (eventually)  
**Interfaces:** CLI, Local Web Dashboard, TUI (future), Obsidian  
**Working Name:** Observr (subject to availability checks)

---

# 1. Executive Summary

Observr is an always-on, local-first personal intelligence system that observes interactions across AI assistants and transforms them into structured, evolving personal knowledge.

The system collects conversation history from platforms such as:

- ChatGPT
- Claude
- Gemini
- Claude Code
- Codex
- Cursor
- OpenCode
- Other supported AI assistants

Instead of simply storing conversations, Observr analyzes them to identify:

- Recurring patterns
- Personal preferences
- Repeated corrections
- Behavioral habits
- Frequently discussed topics
- Decision-making patterns
- Evolving interests
- Project histories
- Forgotten ideas
- Repeated workflows
- Sources of friction
- Changes in preferences over time
- Significant discoveries

These findings are organized into a persistent, evidence-backed knowledge system.

Observr can automatically maintain a dedicated Obsidian vault containing daily summaries, personal profiles, project histories, observations, decisions, and meaningful insights.

Users can explore this information using a CLI, terminal interface, or local web dashboard.

**The ultimate goal is to turn fragmented AI interactions into an evolving, transparent, user-owned personal intelligence system.**

---

# 2. The Problem

People increasingly use multiple AI assistants throughout their daily lives.

They discuss:

- Programming
- Personal projects
- Career decisions
- Financial planning
- Travel
- Learning
- Research
- Relationships
- Ideas
- Goals
- Daily problems
- Personal preferences

However, these interactions are fragmented across multiple applications.

Each provider typically maintains its own history, memory, and search functionality.

The user has no unified view of their conversations.

More importantly, valuable information remains buried in thousands of messages.

## 2.1 Lost Knowledge

Users repeatedly discuss ideas, make decisions, and discover useful information.

Weeks or months later, they may not remember those conversations.

Example:

A user discusses building an application in January, revisits the idea in March, and explores a different implementation in June.

Today, these conversations may exist independently across several AI applications.

Observr should connect them into one coherent project history.

## 2.2 Repeated Corrections

Users frequently correct AI assistants.

Examples:

- "Stop creating unnecessary Markdown files."
- "Don't introduce additional dependencies."
- "Reuse existing components."
- "Keep implementations simple."
- "Don't rewrite working code."
- "Give me shorter explanations."

These corrections often disappear into conversation histories.

Users may not realize how frequently they repeat themselves.

Observr should discover these patterns and provide supporting evidence.

## 2.3 Invisible Habits

AI conversations contain observable patterns in how people work.

For example:

- Frequently reconsidering architectural decisions.
- Requesting simpler solutions after implementations.
- Researching competitors before starting projects.
- Repeatedly discussing the same ideas.
- Preferring certain technologies.
- Revisiting unresolved decisions.
- Asking similar questions across multiple assistants.

Individual conversations may not reveal these patterns.

Analyzing interactions longitudinally can.

Observr should identify meaningful recurring behaviors without making unsupported psychological judgments.

## 2.4 Fragmented Personal Memory

Different AI assistants may know different things about a user.

A preference expressed in ChatGPT may not be available in Claude.

A project decision discussed in Codex may not be reflected elsewhere.

Observr should provide a provider-independent record of relevant knowledge that the user can inspect, modify, export, and selectively share.

## 2.5 No Historical Understanding

Traditional memory systems focus on remembering the latest information.

They often fail to explain:

- When a preference appeared.
- Why a decision was made.
- Whether a preference changed.
- What alternatives were considered.
- Which conversations support an observation.
- Whether information is still relevant.

Observr should preserve the evolution of knowledge, not merely its latest state.

---

# 3. Product Vision

## 3.1 One-Liner

**Observr is a personal intelligence layer that continuously transforms your AI conversations into an evolving understanding of your interests, habits, decisions, and work.**

## 3.2 Extended Pitch

Every conversation with AI contains information about what you're working on, what matters to you, and how your ideas evolve.

Observr connects these conversations across AI platforms, discovers meaningful patterns, and automatically maintains a personal knowledge base.

It runs locally, requires minimal intervention, and gives you complete control over the information it collects.

## 3.3 Product Philosophy

Observr should help users understand themselves and their history.

It should not claim to know users better than they know themselves.

Its observations must be:

1. Evidence-backed
2. Explainable
3. Correctable
4. Traceable
5. Privacy-conscious
6. Useful
7. Non-intrusive

Observr should distinguish between what someone explicitly states and what the system infers.

---

# 4. Core Principles

## 4.1 Local-First

User conversations and extracted knowledge should remain on the user's device by default.

The application must not require a cloud account.

Users should retain ownership of their data.

## 4.2 Passive Observation

After setup, Observr should require little ongoing interaction.

The intended workflow:

1. User enables supported conversation sources.
2. Observr collects new conversations.
3. Observr processes them incrementally.
4. Observr discovers relevant information.
5. Observr updates the knowledge base.
6. User reviews insights when desired.

The collection mechanisms must be explicit and permission-based.

## 4.3 Platform Independence

Observr should support different AI platforms through independent adapters.

The analysis engine should operate on a normalized conversation format.

Adding another provider should not require rewriting the intelligence engine.

## 4.4 Evidence-Based Intelligence

Every generated observation must reference its supporting conversations.

Example:

"User prefers simple implementations."

This observation should include:

- Supporting messages
- Source platform
- Date first observed
- Date last observed
- Number of distinct supporting conversations
- Contradictory evidence, when available
- Whether the user explicitly confirmed it

Unverifiable observations should not become established profile facts.

## 4.5 Human Control

Users must be able to:

- Accept observations
- Reject observations
- Correct observations
- Delete observations
- Exclude topics
- Exclude conversations
- Exclude projects
- Pause collection
- Delete stored data
- Export their knowledge

## 4.6 Useful Intelligence Over Analytics

Observr should prioritize meaningful discoveries over vanity metrics.

"142 prompts sent" is information.

"You've reconsidered the same decision five times across three months" is potentially useful intelligence.

The latter is the product's primary value.

## 4.7 Incremental Knowledge

Observr should not recreate an entire personal profile every time a new conversation appears.

Instead, it should incrementally update the relevant records.

Historical evidence and previous states should remain available unless explicitly deleted.

---

# 5. Target Users

## 5.1 Primary Audience

People who use multiple AI assistants frequently.

Examples:

- Developers
- AI power users
- Researchers
- Students
- Entrepreneurs
- Knowledge workers
- Indie hackers
- Technical professionals

## 5.2 Secondary Audience

People who use AI for personal planning, learning, journaling, and exploring ideas.

## 5.3 Ideal Initial User

A developer who regularly uses:

- ChatGPT for research and discussions
- Claude for brainstorming
- Codex or Claude Code for programming
- Obsidian for knowledge management

This user has hundreds or thousands of AI conversations containing potentially valuable information.

---

# 6. Product Experience

## 6.1 Installation

Proposed future installation:

```bash
brew install observr
```

Alternative:

```bash
npm install -g observr
```

The installation mechanism will be finalized before publishing.

## 6.2 Initialization

```bash
observr init
```

Example setup:

```text
╭─────────────────────────────────────────────╮
│ OBSERVR                                     │
│ Your Personal Intelligence Layer            │
╰─────────────────────────────────────────────╯

Welcome to Observr.

Detected local AI applications:

  ✓ Claude Code
  ✓ Codex
  ○ OpenCode
  ○ Cursor

Conversation import options:

  ○ Import ChatGPT export
  ○ Import Claude export
  ○ Import Gemini export

Knowledge storage:

  ● Create dedicated Obsidian vault
  ○ Use existing Obsidian vault
  ○ Markdown folder only

Analysis engine:

  ● Local model (Ollama)
  ○ Cloud model (API key required)
  ○ Configure later

Background observation:

  ● Enable supported local sources
  ○ Manual import only

Setup complete.
```

A source is only marked as connected after access has been verified.

Not every provider supports continuous capture.

The application must clearly distinguish between:

- Live observation
- Periodic local ingestion
- Manual export import
- Unsupported collection methods

## 6.3 CLI Commands

Proposed interface:

```bash
observr init
observr status
observr import <file>
observr analyze
observr discover
observr profile
observr patterns
observr timeline
observr today
observr week
observr ask "What do I repeatedly correct?"
observr vault sync
observr web
observr doctor
observr export
observr delete
```

Future daemon commands:

```bash
observr start
observr stop
observr restart
observr logs
```

Future TUI:

```bash
observr tui
```

The CLI should remain useful without the web interface.

---

# 7. Core Features

## 7.1 Universal Conversation Collection

### Objective

Collect supported conversation data across multiple AI platforms.

### Initial Sources

- ChatGPT exports
- Claude exports
- Gemini exports
- Codex local sessions
- Claude Code local sessions

### Future Sources

- ChatGPT web interface
- Claude web interface
- Gemini web interface
- Cursor
- OpenCode
- Other developer agents
- Additional AI providers

### Collection Methods

#### Method A: Provider Export Import

Import conversation archives obtained through official provider export mechanisms.

Advantages:

- No browser extension required.
- Access to historical conversations.
- Good for initial analysis.
- No need to intercept application traffic.

Disadvantages:

- Not real-time.
- Export formats may change.
- Users must request or download exports.
- Some providers may omit certain data.

#### Method B: Local Session Adapters

Read documented or accessible local session records from coding-agent tools.

Potential sources:

- Codex local sessions
- Claude Code session files
- Supported agent event hooks

Advantages:

- Near-real-time possibilities.
- No browser extension.
- Useful for developers.

Disadvantages:

- Implementation differs across providers.
- Local data formats may change.
- Some messages may be unavailable.

#### Method C: Browser Extension

An explicitly authorized browser extension captures supported AI web conversations.

Requirements:

- Per-site permission controls.
- Clearly visible collection status.
- No unrelated browser monitoring.
- No keystroke logging.
- No interception of private authentication traffic.
- No undocumented private API dependencies where avoidable.
- Ability to pause capture.
- Duplicate detection.
- Support for provider UI changes.

Native desktop and mobile applications should not be assumed accessible through this extension.

#### Method D: Official APIs

Use provider APIs only where they genuinely expose the required conversation data with user authorization.

Do not assume that a model API provides access to conversations held in the provider's consumer application.

---

# 8. Intelligence Engine

This is the most important component of Observr.

The intelligence engine must transform unstructured conversations into reliable structured knowledge.

## 8.1 Intelligence Categories

### A. Explicit Preferences

Things users directly state.

Examples:

- "I prefer TypeScript."
- "Keep your explanations concise."
- "I don't like overly complicated architectures."
- "I prefer traveling in warm weather."

These should be stored with explicit source evidence.

### B. Inferred Preferences

Preferences suggested by recurring behavior.

Example:

Across multiple conversations, the user repeatedly asks for simpler implementations.

Possible observation:

"The user may prefer relatively simple implementations over additional abstractions."

Inferred preferences must be labeled as such.

### C. Repeated Corrections

Detect recurring feedback directed at AI assistants.

Examples:

- Avoid unnecessary files.
- Don't introduce dependencies.
- Stop asking unnecessary questions.
- Reuse existing components.

Group semantically equivalent corrections.

Track:

- Number of occurrences
- Distinct conversations
- Platforms
- Projects
- First observed
- Last observed
- Supporting evidence

### D. Recurring Interests

Identify topics repeatedly discussed.

Examples:

- AI agents
- Travel
- Financial planning
- Programming languages
- Business ideas
- Personal projects

Distinguish temporary research from sustained interests.

### E. Behavioral Patterns

Identify recurring interaction patterns.

Examples:

- Repeatedly requests simpler solutions.
- Often asks for competitor research before developing products.
- Frequently revisits previous decisions.
- Often requests comparisons before choosing a tool.

These are observations about recorded interactions, not psychological assessments.

### F. Decision History

Track meaningful decisions and their evolution.

Example:

```text
PROJECT: Personal Application

January:
  PostgreSQL selected.

March:
  DynamoDB considered.

April:
  PostgreSQL reaffirmed.

Current:
  PostgreSQL

Evidence:
  3 conversations
```

The system should distinguish:

- Proposal
- Consideration
- Decision
- Reconsideration
- Reversal
- Confirmation
- Unknown current state

### G. Project Intelligence

Identify conversations relating to the same project.

Automatically maintain:

- Project descriptions
- Technical decisions
- Goals
- Open questions
- Important discoveries
- Timeline
- Relevant conversations
- Changes in direction

### H. Forgotten Ideas

Identify ideas or plans repeatedly discussed but not recently mentioned.

Example:

"You previously explored building a finance application across four conversations."

The system must not assume the idea was abandoned.

It can simply indicate that no recent discussion was observed.

### I. Friction Analysis

Identify situations where interactions require repeated clarification or correction.

Examples:

- Repeatedly requesting the same change.
- Correcting misunderstood requirements.
- Reversing unwanted changes.
- Repeatedly narrowing overly broad suggestions.

### J. Workflow Discovery

Identify repeated sequences of requests.

Example:

The user frequently asks an agent to:

1. Research existing solutions.
2. Compare competitors.
3. Design an architecture.
4. Create a PRD.
5. Start implementation.

Observr may suggest creating a reusable workflow.

### K. Evolving Knowledge

Track changes in preferences, opinions, and priorities.

Example:

```text
TOPIC: Development Style

Earlier:
  Preferred detailed implementation plans.

Recently:
  Frequently requests concise, implementation-focused instructions.

Status:
  Possible context-dependent preference shift.
```

The system must avoid interpreting every change as a contradiction.

---

# 9. Intelligence Processing Pipeline

## 9.1 Overview

```text
Conversation Sources
        │
        ▼
Ingestion Adapters
        │
        ▼
Normalization
        │
        ▼
Deduplication
        │
        ▼
Local Conversation Store
        │
        ▼
Structured Extraction
        │
        ▼
Entity & Topic Resolution
        │
        ▼
Pattern Detection
        │
        ▼
Knowledge Reconciliation
        │
        ▼
Insight Generation
        │
        ▼
User Review
        │
        ▼
Obsidian / Dashboard / CLI
```

## 9.2 Stage 1 — Ingestion

Each source adapter imports conversations into a normalized format.

Store:

- Provider
- Source conversation ID
- Conversation title
- Timestamps
- Message roles
- Message content
- Source metadata
- Import timestamp

Conversation timestamps must preserve timezone information when available.

Unknown metadata must remain unknown rather than being invented.

## 9.3 Stage 2 — Deduplication

The same conversation may be imported multiple times.

The system must avoid duplicate storage and analysis.

Use:

- Provider IDs where available.
- Stable normalized message identifiers.
- Content fingerprints as a fallback.
- Source-aware deduplication rules.

Re-importing an unchanged archive should not duplicate observations.

## 9.4 Stage 3 — Structured Extraction

Extract candidate observations from individual conversations.

Example:

```json
{
  "type": "explicit_preference",
  "subject": "user",
  "topic": "software_design",
  "value": "prefers_simple_implementations",
  "source_provider": "chatgpt",
  "evidence_message_ids": [
    "msg_001",
    "msg_014"
  ],
  "extraction_status": "candidate"
}
```

The extraction engine must return structured, validated data.

Use schemas to reject malformed model outputs.

## 9.5 Stage 4 — Entity Resolution

Determine whether different conversations refer to the same underlying entity.

Examples:

- The same personal project.
- The same technical decision.
- The same preference.
- The same recurring correction.

Use deterministic matching where possible.

Use semantic matching when necessary.

Ambiguous matches should remain separate or require review.

## 9.6 Stage 5 — Pattern Detection

Group related observations.

Examples:

```text
"Don't make extra files."
"Stop creating markdown documentation."
"Why did you generate another README?"
```

Potential common pattern:

```text
Avoid unnecessary documentation files.
```

The engine should calculate:

- Number of occurrences
- Number of distinct sessions
- Time range
- Source diversity
- Recent frequency
- Evidence quality

Repeated statements in one conversation should not automatically count as independent evidence.

## 9.7 Stage 6 — Knowledge Reconciliation

Compare newly extracted information with existing knowledge.

Possible outcomes:

- New observation
- Additional supporting evidence
- Contradictory evidence
- Updated preference
- Decision change
- Duplicate information
- Superseded information
- Unresolved ambiguity

Never silently overwrite important historical knowledge.

## 9.8 Stage 7 — Insight Generation

Generate a small number of useful discoveries.

Prioritize:

1. Clear evidence.
2. Meaningful recurrence.
3. Novelty to the user.
4. Potential usefulness.
5. Actionability.
6. Recency.
7. Diversity of supporting conversations.

Avoid generic conclusions.

Bad insight:

"You are interested in technology."

Better insight:

"You've evaluated three different approaches to the same project over two months, and your main concern shifted from implementation difficulty to market competition."

Each insight must contain links to supporting evidence.

---

# 10. Living Personal Profile

## 10.1 Objective

Maintain a continuously evolving record of what the user has explicitly shared and what recurring interaction patterns suggest.

The profile must remain transparent and editable.

## 10.2 Profile Categories

### Preferences

- Technical preferences
- Communication preferences
- Learning preferences
- Work preferences
- Content preferences

### Interests

- Long-term interests
- Recently emerging interests
- Frequently explored topics

### Goals

Only goals supported by explicit user statements.

Examples:

- Build a particular application.
- Learn a new programming language.
- Complete a project.

### Working Habits

Patterns observable from supported interactions.

### Decision Tendencies

Recurring approaches to making decisions.

Avoid unsupported claims about psychological traits.

### Recurring Frustrations

Repeatedly corrected or rejected behaviors.

### Projects

Current and historical projects discussed with AI.

## 10.3 Observation Lifecycle

Each observation can be:

```text
CANDIDATE
    │
    ▼
SUPPORTED
    │
    ├──► USER_CONFIRMED
    │
    ├──► USER_REJECTED
    │
    ├──► SUPERSEDED
    │
    └──► DISMISSED
```

Important:

A high number of repeated mentions should not automatically turn an inference into an unquestionable fact.

## 10.4 Profile Evolution

Each observation should preserve:

- Creation date
- Last observed date
- Evidence references
- Revision history
- User feedback
- Current status
- Relevant context
- Contradictory evidence

Example:

```markdown
# Preference: Simpler Implementations

Status: Supported
Type: Inferred Preference

First observed: 2026-01-10
Last observed: 2026-10-19

## Observation

The user frequently requests simpler
implementations when proposed solutions
introduce additional abstraction.

## Evidence

- [[Conversation-001]]
- [[Conversation-018]]
- [[Conversation-094]]

## Context

Primarily observed in programming discussions.

## Notes

This does not necessarily apply to every
technical design decision.
```

---

# 11. Pattern Discovery

## 11.1 Objective

Detect recurring behavior across multiple conversations.

## 11.2 Pattern Types

- Repeated corrections
- Repeated questions
- Recurring ideas
- Recurring topics
- Repeated decision changes
- Repeated workflows
- Recurring frustrations
- Explicitly stated preferences
- Common task sequences

## 11.3 Pattern Requirements

Every pattern should include:

```text
Pattern ID
Title
Description
Category
First observed
Last observed
Occurrence count
Distinct conversation count
Relevant projects
Relevant platforms
Evidence links
Status
```

Optional:

- Suggested action
- Change over time
- Associated profile observations

## 11.4 Example

```text
PATTERN: Unnecessary Abstraction Corrections

Occurrences: 14
Distinct conversations: 11
Providers: 2

First observed: March 14
Last observed: October 19

Observation:

You frequently ask coding agents to simplify
implementations after an initial solution.

Suggested actions:

[1] Review evidence
[2] Add to profile
[3] Generate reusable agent instruction
[4] Dismiss
```

Counts and dates must be derived from stored evidence.

---

# 12. Decision Timeline

## 12.1 Objective

Track how decisions evolve.

Unlike conventional memory systems, Observr should preserve historical states.

## 12.2 Example

```text
DECISION: Database Selection

January 12
  PostgreSQL selected.

March 18
  DynamoDB evaluated.

April 6
  PostgreSQL confirmed.

Current State:
  PostgreSQL selected.

Evidence:
  3 conversations.
```

## 12.3 Decision Data

Each decision should contain:

- Decision ID
- Topic
- Project
- Candidate options
- Current known state
- Timeline events
- Explicit reasoning
- Supporting conversations
- Last updated timestamp

Do not treat a discussion of an alternative as a confirmed change.

---

# 13. Automatic Journaling

## 13.1 Objective

Generate useful summaries of the user's AI interactions.

## 13.2 Daily Summaries

Generated automatically from available conversations.

Contents:

- Topics discussed
- Projects worked on
- Important decisions
- New discoveries
- Repeated corrections
- Open questions
- Explicit follow-up intentions
- Links to source conversations

Example:

```markdown
# 2026-10-19

## Overview

Conversations: 12
Platforms: ChatGPT, Codex

## Projects

### Observr

- Explored universal AI observability.
- Discussed background monitoring.
- Designed Obsidian integration.
- Defined the intelligence engine.

## Important Decisions

- Selected a local-first approach.
- Chose Obsidian-compatible Markdown.
- Prioritized evidence-backed observations.

## Discovered Patterns

- Repeatedly researching competitors
  before developing new tools.

## Open Questions

- Final project name.
- Preferred analysis model.
- Dashboard design.

## Sources

- [[Conversation-001]]
- [[Conversation-002]]
```

## 13.3 Weekly Summaries

Weekly summaries focus on trends.

Examples:

- Most discussed projects
- Significant decisions
- Recurring topics
- New preferences
- Important changes
- Insights worth revisiting

## 13.4 Monthly Summaries

Monthly summaries focus on longer-term evolution.

Examples:

- New interests
- Changes in priorities
- Recurring patterns
- Project evolution
- Significant decisions
- Observations confirmed or superseded

## 13.5 Accuracy Requirements

A summary must not claim that an action occurred merely because it was discussed.

For example:

"Discussed implementing authentication"

is different from:

"Implemented authentication."

Only use the second when the source provides sufficient evidence.

---

# 14. Obsidian Integration

## 14.1 Objective

Automatically maintain a dedicated Markdown knowledge base compatible with Obsidian.

Obsidian should be optional.

The system must work with ordinary Markdown folders.

## 14.2 Default Vault Structure

```text
Observr/
│
├── Home.md
│
├── Daily/
│   ├── 2026-10-18.md
│   └── 2026-10-19.md
│
├── Weekly/
│   └── 2026-W43.md
│
├── Monthly/
│   └── 2026-10.md
│
├── Profile/
│   ├── Overview.md
│   ├── Preferences.md
│   ├── Interests.md
│   ├── Working-Habits.md
│   ├── Communication-Style.md
│   └── Goals.md
│
├── Patterns/
│   ├── Repeated-Corrections.md
│   ├── Recurring-Questions.md
│   ├── Recurring-Workflows.md
│   └── Common-Frustrations.md
│
├── Projects/
│   ├── Project-A.md
│   └── Project-B.md
│
├── Decisions/
│   └── Database-Selection.md
│
├── Insights/
│   └── Discoveries.md
│
├── Sources/
│   └── Conversations/
│
└── Views/
    ├── Patterns.base
    └── Decisions.base
```

The exact organization can evolve.

The system should avoid generating thousands of unnecessary files.

## 14.3 Obsidian Features

Support:

- Markdown notes
- YAML frontmatter
- Wiki links
- Backlinks
- Daily notes
- Project notes
- Optional Obsidian Bases
- Source references
- Graph navigation

## 14.4 Example Pattern Note

```markdown
---
id: pattern-001
type: recurring_pattern
status: supported
first_observed: 2026-03-14
last_observed: 2026-10-19
occurrences: 14
---

# Preference for Simpler Implementations

## Summary

Across multiple programming conversations,
the user requested simpler implementations
after receiving an initial solution.

## Evidence

- [[Conversation-001]]
- [[Conversation-018]]
- [[Conversation-094]]

## Related

- [[Engineering-Preferences]]
- [[Repeated-Corrections]]

## Potential Action

Consider creating a reusable instruction
for supported AI coding agents.
```

## 14.5 Synchronization Rules

SQLite stores the canonical conversation evidence and structured observations.

The Obsidian vault is a human-readable knowledge projection.

However, users must be able to edit notes.

To avoid overwriting changes:

- Generated sections should have stable identifiers.
- User-authored content should be preserved.
- Manual corrections should be imported as feedback.
- Conflicting changes should trigger review.
- Deleted observations should not reappear without new evidence and a clear review process.
- Regeneration should be idempotent.

## 14.6 Important Design Decision

**Do not use Obsidian as the primary event database.**

Use SQLite for structured records and Markdown for human-readable knowledge.

This provides better querying, indexing, consistency, and performance.

---

# 15. Local Web Dashboard

## 15.1 Objective

Provide an interactive way to explore collected conversations and generated knowledge.

Proposed address:

```text
http://127.0.0.1:4317
```

The server should bind only to localhost by default.

## 15.2 Main Sections

### Overview

Display:

- Recent discoveries
- Recent activity
- Updated profile observations
- Recent decisions
- Knowledge vault status

The primary focus should be discoveries, not charts.

### Patterns

Explore recurring:

- Corrections
- Questions
- Workflows
- Topics
- Behaviors

Allow filtering by:

- Platform
- Time range
- Project
- Pattern category

### Profile

Display the evolving personal profile.

Each observation must be inspectable and correctable.

### Timeline

Visualize:

- Important conversations
- Decisions
- Project evolution
- Preference changes
- Discoveries

### Knowledge

Browse generated Markdown knowledge.

### Sources

Search and inspect the original conversations behind insights.

### Settings

Configure:

- Conversation sources
- Privacy
- Analysis engine
- Collection exclusions
- Scheduling
- Vault location
- Retention policy

---

# 16. Terminal User Interface

## 16.1 Objective

Provide fast navigation and insight review directly in the terminal.

The TUI is a future feature.

## 16.2 Example

```text
╭──────────────────────────────────────────────╮
│ OBSERVR                           ● ACTIVE   │
├──────────────────────────────────────────────┤
│                                              │
│ TODAY                                        │
│                                              │
│ Conversations                  12            │
│ New observations               5            │
│ Patterns updated               3            │
│                                              │
│ RECENT DISCOVERIES                           │
│                                              │
│ 1. Repeated architecture corrections         │
│ 2. Recurring project idea                    │
│ 3. Possible preference change                │
│                                              │
│ PROFILE                                      │
│                                              │
│ Supported preferences          24            │
│ Recurring patterns             11            │
│ Tracked decisions              18            │
│                                              │
│ [P] Patterns  [I] Insights  [S] Search        │
╰──────────────────────────────────────────────╯
```

## 16.3 TUI Technology

Potential libraries:

- Ink
- OpenTUI
- Bubble Tea if using Go
- Ratatui if moving to Rust

For the initial TypeScript implementation, prefer a mature terminal UI library that integrates cleanly with the core application.

---

# 17. Ask Your History

## 17.1 Objective

Allow users to ask questions about their historical AI interactions.

Examples:

```bash
observr ask "What do I repeatedly correct?"
```

```bash
observr ask "What projects have I discussed this year?"
```

```bash
observr ask "How has my development style changed?"
```

```bash
observr ask "What decisions did I reconsider?"
```

```bash
observr ask "What ideas have I discussed but not revisited?"
```

## 17.2 Requirements

Answers must:

- Search relevant stored conversations.
- Use evidence-backed observations.
- Link to original messages.
- Distinguish facts from inferences.
- Disclose incomplete source coverage.
- Avoid inventing missing information.

## 17.3 Retrieval Approach

Start with SQLite full-text search.

Future enhancements:

- Semantic search
- Embeddings
- Hybrid retrieval
- Temporal retrieval
- Project-aware search

Embeddings should not be mandatory for the MVP.

---

# 18. Technical Architecture

## 18.1 High-Level Design

```text
                   AI PLATFORMS
┌──────────────────────────────────────────┐
│                                          │
│  ChatGPT    Claude    Gemini             │
│  Codex      Claude Code                  │
│  Cursor     OpenCode                     │
│                                          │
└──────────────────┬───────────────────────┘
                   │
                   ▼
         ┌──────────────────────┐
         │ INGESTION ADAPTERS   │
         │                      │
         │ - Export Importers   │
         │ - Local File Readers │
         │ - Browser Extension  │
         │ - Agent Hooks        │
         └──────────┬───────────┘
                    │
                    ▼
         ┌──────────────────────┐
         │ BACKGROUND DAEMON    │
         │                      │
         │ - Event Processing   │
         │ - Deduplication      │
         │ - Scheduling         │
         │ - Source Monitoring  │
         └──────────┬───────────┘
                    │
                    ▼
         ┌──────────────────────┐
         │ LOCAL DATA STORE     │
         │                      │
         │ SQLite               │
         │ Full-Text Search     │
         └──────────┬───────────┘
                    │
                    ▼
         ┌──────────────────────┐
         │ INTELLIGENCE ENGINE  │
         │                      │
         │ Extraction           │
         │ Entity Resolution    │
         │ Pattern Discovery    │
         │ Decision Tracking    │
         │ Profile Evolution    │
         │ Insight Generation   │
         └──────────┬───────────┘
                    │
         ┌──────────┴───────────┐
         │                      │
         ▼                      ▼
┌───────────────────┐  ┌──────────────────────┐
│ KNOWLEDGE VAULT   │  │ INTERFACES           │
│                   │  │                      │
│ Obsidian Markdown │  │ CLI                  │
│ Daily Journals    │  │ Web Dashboard        │
│ Profile           │  │ TUI                  │
│ Projects          │  │ Search               │
│ Decisions         │  │                      │
└───────────────────┘  └──────────────────────┘
```

## 18.2 Architecture Principles

- Modular adapters
- Provider-independent internal schema
- Incremental processing
- Idempotent ingestion
- Versioned analysis
- Evidence preservation
- Privacy by default
- Explicit user control
- Replaceable AI models
- Separation of collection and analysis
- Separation of data and presentation

---

# 19. Technology Stack

## 19.1 Recommended Stack

| Component | Technology |
|---|---|
| Language | TypeScript |
| Runtime | Node.js |
| Package Manager | pnpm |
| Database | SQLite |
| Database Access | Drizzle ORM / SQLite driver |
| Search | SQLite FTS5 |
| Schema Validation | Zod |
| CLI | Commander or similar |
| Dashboard | React |
| Frontend Build | Vite |
| Styling | Tailwind CSS |
| Local AI | Ollama |
| Remote AI | Provider adapters |
| Browser Extension | TypeScript |
| Testing | Vitest |
| End-to-End Testing | Playwright |
| Knowledge Storage | Markdown + YAML |
| Background Service | launchd / systemd / platform equivalents |

## 19.2 Why TypeScript?

Advantages:

- Rapid iteration
- Shared types
- Large ecosystem
- Easy browser extension development
- Straightforward dashboard integration
- Good support for AI provider SDKs

Rust could eventually be used for performance-sensitive components.

It is not required for the first version.

## 19.3 Why SQLite?

Advantages:

- Embedded database
- No external service
- Portable
- Reliable transactions
- Good query performance
- Full-text search
- Easy local backup

Sensitive data must be protected using a deliberate encryption strategy.

Plain SQLite does not provide database encryption automatically.

---

# 20. Data Model

The data model must support provenance, history, and corrections.

## 20.1 Core Entities

### Sources

Represents an AI platform or local data source.

Fields:

```text
id
provider
source_type
display_name
enabled
last_sync_at
created_at
```

### Conversations

Represents a conversation.

Fields:

```text
id
source_id
external_id
title
created_at
updated_at
imported_at
content_fingerprint
metadata
```

### Messages

Represents an individual message.

Fields:

```text
id
conversation_id
external_id
role
content
created_at
sequence_number
metadata
```

### Observations

Structured claims extracted from conversations.

Fields:

```text
id
type
subject
topic
canonical_value
display_text
status
inference_type
context
first_observed_at
last_observed_at
created_at
updated_at
```

### Evidence

Links observations to their source messages.

Fields:

```text
id
observation_id
conversation_id
message_id
evidence_type
quote
created_at
```

### Patterns

Represents recurring observations.

Fields:

```text
id
type
title
description
status
first_observed_at
last_observed_at
occurrence_count
distinct_conversation_count
created_at
updated_at
```

### Pattern Evidence

Links patterns to observations or messages.

Fields:

```text
id
pattern_id
observation_id
message_id
```

### Decisions

Represents an evolving decision.

Fields:

```text
id
topic
project_id
current_state
created_at
updated_at
```

### Decision Events

Represents a change in the decision timeline.

Fields:

```text
id
decision_id
event_type
description
occurred_at
source_conversation_id
source_message_id
```

### Projects

Represents identified projects.

Fields:

```text
id
name
description
status
created_at
updated_at
```

### Insights

Represents discoveries surfaced to the user.

Fields:

```text
id
title
description
category
status
created_at
reviewed_at
```

### Insight Evidence

Links discoveries to their supporting records.

### User Feedback

Represents accepted, rejected, or corrected observations.

Fields:

```text
id
target_type
target_id
action
user_value
created_at
```

### Processing Checkpoints

Tracks completed imports and analysis runs.

Fields:

```text
id
source_id
processor_name
processor_version
checkpoint
processed_at
```

## 20.2 Additional Design Requirements

Use stable internal identifiers.

Maintain source references even when data is reprocessed.

Support schema migrations.

Store analysis model and prompt versions for derived observations.

Use appropriate indexes.

Support deletion across dependent derived records.

Do not persist the same conversation repeatedly after re-import.

---

# 21. Background Daemon

## 21.1 Objective

Continuously process supported conversation sources without requiring an open terminal.

## 21.2 Responsibilities

- Monitor supported local session files.
- Receive browser extension events.
- Schedule incremental analysis.
- Process newly imported conversations.
- Update observations and patterns.
- Generate daily and weekly summaries.
- Synchronize the knowledge vault.
- Maintain checkpoints.
- Expose local status information.

## 21.3 Operational Requirements

- Low idle CPU usage
- Configurable analysis schedule
- Graceful restart
- Safe shutdown
- Crash recovery
- No duplicated processing
- Structured logs
- Health/status commands

## 21.4 Important Limitation

An always-on daemon cannot automatically access every native AI application.

Collection requires a supported adapter, export, hook, extension, or authorized integration.

The product must communicate this clearly.

---

# 22. AI Analysis

## 22.1 Model Independence

Observr should not depend on one specific LLM provider.

Support a common model interface.

Initial options:

- Ollama
- OpenAI-compatible APIs
- Anthropic API
- Other provider adapters later

## 22.2 Analysis Modes

### Local Mode

Analysis occurs using a locally installed model.

Advantages:

- Better privacy control
- No external inference API required
- Potentially lower marginal cost

Disadvantages:

- Hardware requirements
- Variable model quality
- Slower processing on some devices

### Cloud Mode

Analysis uses a selected external AI provider.

Advantages:

- Potentially stronger extraction
- Less local computational load

Disadvantages:

- API cost
- Data transmission
- Privacy considerations

Cloud analysis must be opt-in.

## 22.3 Processing Efficiency

Avoid reanalyzing entire conversation histories.

Use:

- Content fingerprints
- Incremental extraction
- Structured observation storage
- Selective reprocessing
- Analysis caching
- Prompt version tracking
- Model version tracking

## 22.4 Model Output Requirements

All structured extraction should use validated schemas.

Malformed responses should be retried or rejected.

Every nontrivial observation should include its source evidence.

Unsupported claims should not be included in the established personal profile.

---

# 23. Privacy & Security

This is a foundational product requirement.

Observr may process highly sensitive personal information.

## 23.1 Core Requirements

### Explicit Consent

Users must explicitly enable each data source.

### Local Storage

User data stays local unless the user explicitly enables remote processing or synchronization.

### Encryption

Protect stored conversations and derived knowledge using an appropriate encryption strategy.

Where supported, use OS-protected keys.

SQLite full-text indexing requires special consideration because search indexes can also contain sensitive content.

### Secret Redaction

Detect and redact likely credentials, tokens, and API keys before analysis or external transmission.

Redaction is not guaranteed to catch every secret.

### Source Exclusion

Users can exclude:

- Specific providers
- Projects
- Conversations
- Directories
- Topics
- Future collection sessions

### Sensitive Information

Do not automatically build inferred profiles of sensitive personal characteristics.

Explicitly disclosed sensitive information should not be added to the long-term profile without user control.

### Deletion

Users must be able to delete:

- Source conversations
- Extracted observations
- Associated patterns
- Related insights
- Generated Markdown projections

Deletion must propagate to derived content.

### Local Web Security

The localhost dashboard must not expose sensitive data to arbitrary local or remote websites.

Use appropriate authentication, origin validation, request protections, and secure defaults.

Bind to localhost unless explicitly configured otherwise.

### No Hidden Telemetry

No automatic collection of user analytics for the project's maintainers.

### No Global Surveillance

Do not implement:

- System-wide keylogging
- Hidden screen recording
- Unrelated browser monitoring
- Private traffic interception
- Unauthorized account access

---

# 24. MVP Definition — Version 0.1

## 24.1 Goal

Prove that Observr can generate genuinely useful personal intelligence from historical AI conversations.

Do not start by building the complete always-on platform.

## 24.2 MVP Features

### Required

1. CLI application.
2. ChatGPT export importer.
3. Normalized conversation storage.
4. SQLite database.
5. Deduplication.
6. Local conversation search.
7. Structured observation extraction.
8. Recurring pattern detection.
9. Personal profile generation.
10. Decision extraction and basic timelines.
11. Daily summary generation.
12. Obsidian-compatible Markdown export.
13. Evidence links.
14. User feedback on observations.
15. Minimal localhost dashboard.

### Optional

- Ollama support.
- Claude export importer.
- Semantic search.
- Weekly summaries.

### Explicitly Out of Scope

- Browser extension.
- Universal live capture.
- Production-grade background daemon.
- Complete TUI.
- Autonomous rule enforcement.
- Direct integrations with every AI provider.
- Mobile application.
- Cloud synchronization.

## 24.3 MVP User Flow

```text
1. Install Observr.

2. Run:
   observr init

3. Import history:
   observr import chatgpt-export.zip

4. Analyze:
   observr analyze

5. Discover:
   observr discover

6. Generate vault:
   observr vault sync

7. Open dashboard:
   observr web

8. Review insights and profile.
```

## 24.4 MVP Success Criteria

The MVP should demonstrate:

- Correct conversation ingestion.
- Safe repeated imports.
- Reliable evidence references.
- Useful recurring pattern discovery.
- Accurate daily summaries.
- Correctable profile observations.
- Usable Obsidian notes.
- No fabricated supporting conversations.

---

# 25. Roadmap

## Phase 0 — Intelligence Proof of Concept

### Deliverables

- ChatGPT export parser.
- Conversation normalizer.
- Structured extraction.
- Basic pattern grouping.
- Evidence-backed personal report.

### Objective

Determine whether meaningful discoveries can be extracted from real conversation histories.

### Exit Criteria

A test user can identify several useful, accurate insights that were not obvious from individual conversations.

---

## Phase 1 — Local Intelligence Engine

### Deliverables

- SQLite persistence.
- Deduplication.
- Full-text search.
- Observation lifecycle.
- Decision tracking.
- Profile evolution.
- User feedback.
- Obsidian vault generation.

### Objective

Build a reliable foundation for incremental knowledge.

---

## Phase 2 — Continuous Collection

### Deliverables

- Background daemon.
- Codex adapter.
- Claude Code adapter.
- Incremental synchronization.
- Scheduled analysis.
- Daily journal updates.

### Objective

Reduce the need for manual imports.

---

## Phase 3 — Universal AI Collection

### Deliverables

- Browser extension.
- ChatGPT web support.
- Claude web support.
- Gemini web support.
- Additional provider adapters.
- Source permission management.

### Objective

Support continuous collection across multiple AI interfaces, where technically and legally feasible.

---

## Phase 4 — Complete Product Interface

### Deliverables

- Polished local dashboard.
- Timeline explorer.
- Pattern explorer.
- Profile editor.
- Evidence viewer.
- Full-text search UI.
- TUI.

### Objective

Make insights easy to explore and understand.

---

## Phase 5 — Advanced Intelligence

### Deliverables

- Cross-platform behavior comparisons.
- More sophisticated decision evolution.
- Long-term preference changes.
- Better recurring workflow detection.
- Personalized recommendations.
- Historical question answering.
- Knowledge relationship visualization.

### Objective

Move from descriptive analytics to meaningful personal intelligence.

---

## Phase 6 — Optional Actions

### Deliverables

- Generate reusable agent instructions.
- Export selected preferences to AGENTS.md.
- Export selected preferences to CLAUDE.md.
- Suggest workflow automations.
- Create reusable prompts.
- Suggest follow-up actions.

### Objective

Allow users to act on discoveries.

No automatic changes to external AI tools without explicit approval.

---

# 26. Suggested Repository Architecture

Use a TypeScript monorepo.

```text
observr/
│
├── README.md
├── PROJECT.md
├── CONTRIBUTING.md
├── LICENSE
├── package.json
├── pnpm-workspace.yaml
├── tsconfig.json
│
├── apps/
│   │
│   ├── cli/
│   │   └── src/
│   │
│   ├── daemon/
│   │   └── src/
│   │
│   ├── dashboard/
│   │   └── src/
│   │
│   └── extension/
│       └── src/
│
├── packages/
│   │
│   ├── core/
│   │   └── src/
│   │
│   ├── database/
│   │   ├── src/
│   │   └── migrations/
│   │
│   ├── ingestion/
│   │   └── src/
│   │
│   ├── adapters/
│   │   ├── chatgpt/
│   │   ├── claude/
│   │   ├── gemini/
│   │   ├── codex/
│   │   └── claude-code/
│   │
│   ├── intelligence/
│   │   ├── extraction/
│   │   ├── patterns/
│   │   ├── decisions/
│   │   ├── profile/
│   │   └── insights/
│   │
│   ├── models/
│   │   ├── ollama/
│   │   └── remote/
│   │
│   ├── search/
│   │   └── src/
│   │
│   ├── vault/
│   │   └── src/
│   │
│   └── shared/
│       └── src/
│
├── tests/
│   ├── fixtures/
│   ├── unit/
│   ├── integration/
│   └── e2e/
│
├── docs/
│   ├── architecture.md
│   ├── data-model.md
│   ├── intelligence.md
│   ├── privacy.md
│   └── roadmap.md
│
└── examples/
    └── sample-vault/
```

For the initial MVP, only implement packages that are actually required.

Avoid creating empty packages merely to match the planned architecture.

---

# 27. Engineering Requirements

## 27.1 Testing

Use automated tests for:

- Import parsing
- Normalization
- Deduplication
- Database migrations
- Evidence linking
- Pattern grouping
- Profile updates
- Decision reconciliation
- Vault generation
- User feedback
- Data deletion

## 27.2 Evaluation Dataset

Create a small synthetic dataset containing:

- Repeated corrections
- Contradictory preferences
- Multiple conversations about one project
- Evolving decisions
- Similar but unrelated topics
- Explicit preferences
- Inferred patterns
- Duplicate conversations

Expected outputs should be manually defined.

## 27.3 Quality Evaluation

Evaluate:

### Factual Grounding

Are observations supported by real messages?

### Precision

How many generated observations are genuinely correct?

### Duplication

Does the engine repeatedly rediscover the same pattern?

### Relevance

Does an insight matter to the user?

### Temporal Accuracy

Does the engine correctly distinguish historical and current decisions?

### User Trust

Can users understand why a conclusion was generated?

## 27.4 Suggested Initial Targets

These are development targets, not established benchmarks.

- At least 90% evidence-grounded factual observations.
- At least 3 of the top 5 discoveries rated useful by test users.
- No duplicate records after repeated imports.
- No fabricated evidence references.
- Successful reconstruction of deliberately created decision histories.
- Correct propagation of user rejection/deletion.

---

# 28. Performance Requirements

The application should remain usable with thousands of conversations.

## 28.1 Processing

- Process new conversations incrementally.
- Avoid repeated full-history analysis.
- Batch expensive operations.
- Support resuming interrupted imports.
- Use checkpoints.
- Cache analysis results.

## 28.2 Storage

- Minimize unnecessary duplication.
- Index frequently searched fields.
- Keep derived data separate from raw source records.
- Support backups and exports.

## 28.3 Daemon

Future daemon requirements:

- Minimal idle CPU usage.
- Configurable analysis frequency.
- No constant LLM inference.
- Graceful recovery.
- Observable processing state.

---

# 29. Differentiation From Existing Products

Observr competes with adjacent products in several categories:

- AI conversation archival
- Personal AI memory
- AI session analytics
- Automatic journaling
- Personal knowledge management
- Obsidian integrations
- Cross-provider conversation search

Relevant projects to study include:

- Retrace
- OpenMem
- RiverHistory
- AI Conversation Extractor
- Screenpipe
- Existing provider-native memory features

The existence of these tools means Observr should not claim to have invented universal conversation capture or AI-generated personal profiles.

## 29.1 Proposed Differentiation

Observr should focus on the combination of:

1. Cross-provider observation.
2. Evidence-backed personal intelligence.
3. Longitudinal knowledge evolution.
4. Explicit and inferred preference separation.
5. Recurring behavior discovery.
6. Decision histories.
7. Automatic knowledge organization.
8. Human-readable Obsidian output.
9. User-controlled profile corrections.
10. Local-first privacy.

## 29.2 Primary Differentiator

**Observr does not simply remember conversations. It discovers meaningful patterns and preserves how knowledge evolves.**

## 29.3 Critical Competitive Test

Before expanding beyond the MVP, compare Observr's discoveries against existing alternatives.

The system should produce meaningfully more useful, accurate, or actionable results.

A more attractive dashboard alone is insufficient differentiation.

---

# 30. Product Positioning

## 30.1 Main Tagline

**Your conversations tell a story. Observr helps you understand it.**

## 30.2 Alternative Taglines

- Your personal intelligence layer.
- Turn AI conversations into living knowledge.
- Discover what your conversations reveal.
- Your AI history, finally connected.
- The missing intelligence layer across your AI tools.
- Your second brain, built from your conversations.

## 30.3 GitHub Description

An open-source, local-first personal intelligence engine that connects your AI conversations, discovers recurring patterns and evolving preferences, and builds an evidence-backed knowledge base.

## 30.4 Marketing Message

You have hundreds or thousands of conversations scattered across different AI assistants.

Those conversations contain ideas, preferences, decisions, goals, and useful discoveries.

Observr connects them.

It identifies what you keep discussing, what you've changed your mind about, what you repeatedly correct, and what you may have forgotten.

Then it turns that information into an evolving knowledge base you actually own.

---

# 31. Example Product Demo

## 31.1 Installation

```bash
observr init
```

## 31.2 Import

```bash
observr import chatgpt-export.zip
```

Example output:

```text
OBSERVR — Conversation Import

Source: ChatGPT

Conversations found: 2,841
Messages found: 38,421

Importing...

✓ Conversations normalized
✓ Duplicates checked
✓ Records stored

Import complete.
```

## 31.3 Analyze

```bash
observr analyze
```

Example output:

```text
OBSERVR — Intelligence Analysis

Processing conversation history...

✓ Topics extracted
✓ Preferences identified
✓ Decisions analyzed
✓ Repeated corrections grouped
✓ Project relationships detected

Analysis complete.
```

## 31.4 Discover

```bash
observr discover
```

Example output:

```text
OBSERVR — Your Discoveries

01. RECURRING IDEA

You've explored the same developer-tool
concept across multiple conversations.

Your requirements evolved from:

  Novelty
    ↓
  Low Competition
    ↓
  Daily Usefulness
    ↓
  Personal AI Intelligence

[View timeline]


02. REPEATED CORRECTION

You've repeatedly asked AI assistants
to avoid unnecessary complexity.

Observed across multiple conversations.

[View evidence]


03. DECISION HISTORY

You've reconsidered a technology choice
several times over recent months.

[Explore decision]
```

All counts and findings must originate from actual imported data.

These examples are illustrative.

## 31.5 Generate Obsidian Vault

```bash
observr vault sync
```

```text
OBSERVR — Knowledge Vault

✓ Daily summaries generated
✓ Personal profile updated
✓ Decision history created
✓ Patterns linked
✓ Project notes created

Vault:
~/Documents/Observr

Open in Obsidian? [Y/n]
```

---

# 32. Project Risks

## 32.1 Low-Quality Insights

Risk:

The system generates obvious, generic, or incorrect observations.

Mitigation:

- Require evidence.
- Rank insights by relevance.
- Evaluate outputs.
- Reduce redundant findings.
- Collect user feedback.
- Prefer fewer, better insights.

## 32.2 Privacy Concerns

Risk:

Users may hesitate to grant access to years of private conversations.

Mitigation:

- Local-first architecture.
- Explicit permissions.
- Encryption.
- Transparency.
- Easy deletion.
- Clear remote-processing controls.

## 32.3 Provider Changes

Risk:

AI providers change their export formats, web interfaces, or local session storage.

Mitigation:

- Modular adapters.
- Versioned parsers.
- Compatibility tests.
- Graceful degradation.

## 32.4 High Processing Cost

Risk:

Analyzing thousands of conversations could become expensive.

Mitigation:

- Incremental processing.
- Caching.
- Configurable models.
- Local inference.
- Batching.
- Selective reanalysis.

## 32.5 Novelty Without Retention

Risk:

Users find the first report fascinating but never return.

Mitigation:

Prioritize recurring utility:

- New insights.
- Meaningful profile changes.
- Decision tracking.
- Useful weekly reviews.
- Searchable personal knowledge.
- Actionable recommendations.

## 32.6 Scope Creep

Risk:

Trying to build every integration and interface before proving the intelligence engine.

Mitigation:

Start with historical imports and one high-quality intelligence report.

---

# 33. Business & Distribution

## 33.1 Initial Distribution

Focus on:

- GitHub
- Hacker News
- Reddit
- AI developer communities
- Obsidian communities
- AI power users

## 33.2 Open-Source Philosophy

Recommended:

- Open-source core.
- Local-first.
- No required subscriptions.
- No required external accounts.
- Transparent data processing.
- Portable knowledge format.

## 33.3 Monetization

Not a priority for the MVP.

Potential future options:

- Optional hosted synchronization.
- Managed encrypted backups.
- Team-oriented features.
- Additional enterprise controls.

Core local functionality should remain useful without paid services.

---

# 34. Development Priorities

## Priority 0 — Foundation

- Establish monorepo.
- Define normalized conversation schema.
- Implement SQLite storage.
- Implement ChatGPT importer.
- Add deduplication.
- Build basic CLI.

## Priority 1 — Intelligence

- Implement structured extraction.
- Add evidence linking.
- Extract explicit preferences.
- Extract decisions.
- Detect recurring corrections.
- Group related observations.
- Generate initial discoveries.

## Priority 2 — Knowledge

- Build evolving profile.
- Implement decision histories.
- Add daily summaries.
- Implement Obsidian vault generation.
- Add user corrections and feedback.

## Priority 3 — Interface

- Implement local dashboard.
- Add discovery review.
- Add timeline visualization.
- Add conversation search.
- Add profile editing.

## Priority 4 — Automation

- Add background daemon.
- Add Codex integration.
- Add Claude Code integration.
- Add automatic daily processing.

## Priority 5 — Universal Capture

- Build browser extension.
- Add supported AI web platforms.
- Add additional provider importers.
- Improve source management.

---

# 35. Suggested First Implementation Tasks

The first development milestone should remain deliberately small.

### Task 1 — Initialize Repository

Create a TypeScript monorepo with:

- CLI package
- Core package
- Database package
- Ingestion package
- Intelligence package
- Vault package
- Shared types

Configure:

- TypeScript
- pnpm
- Testing
- Linting
- Formatting

### Task 2 — Define Conversation Schema

Implement normalized types for:

- Sources
- Conversations
- Messages
- Metadata
- Import checkpoints

### Task 3 — Implement ChatGPT Importer

Requirements:

- Accept supported ChatGPT export archives.
- Parse conversation records.
- Preserve timestamps.
- Preserve roles.
- Handle missing metadata.
- Avoid duplicates.

### Task 4 — Implement SQLite Persistence

Create:

- Database schema
- Migrations
- Repositories
- Required indexes

### Task 5 — Implement Basic Search

Support:

```bash
observr search "PostgreSQL"
```

Return relevant conversation references.

### Task 6 — Implement Observation Extraction

Extract:

- Explicit preferences
- Decisions
- Corrections
- Topics
- Project references

Require source evidence.

### Task 7 — Implement Pattern Grouping

Group repeated observations.

Start with corrections and explicit preferences.

### Task 8 — Implement First Intelligence Report

Support:

```bash
observr discover
```

Return the highest-quality findings with source references.

### Task 9 — Generate Markdown Knowledge

Support:

```bash
observr vault sync
```

Generate:

- Home page
- Profile
- Pattern notes
- Decision notes
- Daily summaries

### Task 10 — Evaluate With Real Data

Test with a substantial conversation history.

Measure:

- Factual accuracy
- Evidence quality
- Duplicate observations
- Insight usefulness
- Processing cost

Do not expand the project until the discoveries demonstrate real value.

---

# 36. Future Possibilities

These are not MVP commitments.

## 36.1 Personal Knowledge Graph

Visualize relationships between:

- Projects
- Topics
- Decisions
- Preferences
- Patterns
- Conversations

## 36.2 Cross-Agent Intelligence

Compare recurring behavior across supported AI tools.

Example:

"You correct one assistant more frequently for unnecessary code changes."

Only produce such findings when the underlying data supports meaningful comparison.

## 36.3 Personalized Agent Instructions

Convert confirmed preferences into reusable instructions.

Example:

```bash
observr export rules --target codex
```

The user reviews the generated instructions before applying them.

## 36.4 Context Portability

Allow users to export selected confirmed knowledge for use with another AI assistant.

Do not automatically expose an entire personal profile.

## 36.5 Personalized Weekly Review

Generate a concise weekly report highlighting:

- New discoveries
- Meaningful changes
- Unresolved questions
- Important project developments
- Repeated friction
- Relevant forgotten ideas

## 36.6 Conversational Historical Search

Allow complex queries such as:

"What did I believe about this project six months ago?"

"What changed my decision?"

"Which ideas have I returned to most often?"

"What preferences have changed since last year?"

## 36.7 Knowledge Export

Support:

- Markdown
- JSON
- Structured profile export
- Selected agent instruction files

## 36.8 Optional MCP Server

Expose explicitly selected knowledge to supported AI agents.

Example:

An AI coding agent requests the user's confirmed development preferences.

Observr returns only the approved, relevant information.

The MCP server must enforce source and scope permissions.

---

# 37. What We Must Not Build

To preserve focus, Observr must not become:

- Another generic chatbot.
- Another task manager.
- Another AI agent orchestrator.
- A screen recorder.
- A keylogger.
- A surveillance application.
- A generic analytics dashboard.
- A simple conversation backup utility.
- A system that makes unsupported personality judgments.
- A memory system that silently invents facts.
- A platform requiring all conversations to be uploaded to a remote server.

The core identity must remain:

**Personal intelligence derived from evidence-backed AI interaction history.**

---

# 38. Key Product Decisions

The following decisions are proposed defaults.

| Decision | Choice |
|---|---|
| Product category | Universal AI observability and personal intelligence |
| Primary interface | CLI + Local Web Dashboard |
| Future interface | TUI |
| Primary storage | SQLite |
| Knowledge format | Markdown |
| Knowledge interface | Obsidian |
| Core language | TypeScript |
| Runtime | Node.js |
| Initial platform | macOS |
| Initial conversation source | ChatGPT export |
| Initial intelligence focus | Patterns, preferences, decisions |
| Analysis model | Provider-neutral, local preferred |
| Background observation | Phase 2 |
| Browser extension | Phase 3 |
| Hosting | Local-first |
| Source evidence | Mandatory |
| User control | Mandatory |
| Open-source | Yes |
| Primary success metric | Useful, accurate discoveries |

---

# 39. Definition of Success

Observr is successful when users can import their conversation histories and discover meaningful information they would otherwise have difficulty identifying.

A successful experience should feel like:

"I forgot I had explored that idea."

"I didn't realize I corrected AI for that so often."

"Interesting, my requirements changed substantially over time."

"Now I understand why I made that decision."

"I can finally find what I discussed months ago."

"My Obsidian knowledge base organizes itself."

The application should deliver these discoveries accurately, transparently, and without requiring extensive manual organization.

---

# 40. Final Product Vision

The long-term vision is an always-on personal intelligence system that exists independently of any single AI provider.

It continuously collects authorized interactions, organizes historical knowledge, detects meaningful patterns, and maintains an evolving understanding of what the user has explicitly shared.

It should understand:

- What users discuss.
- What they repeatedly ask.
- What they explicitly prefer.
- What recurring behavior suggests.
- What they have decided.
- What they have reconsidered.
- What projects they are exploring.
- How their interests evolve.
- Which ideas keep returning.
- What they may have forgotten.

Every important observation should be supported by evidence.

Users should remain in control of their personal data and the conclusions drawn from it.

The system should be useful without requiring users to change how they interact with AI.

It should work quietly in the background, continuously transforming fragmented conversations into organized knowledge.

**Observr is not intended to replace AI assistants.**

**It is intended to give users ownership, visibility, and understanding of the knowledge accumulated through their interactions with them.**

---

# 41. Immediate Next Step

Before implementing the daemon, browser extension, or advanced dashboard, build the smallest version that can answer one question:

**Can Observr discover meaningful, accurate, and useful patterns from a person's existing AI conversation history?**

The first milestone is therefore:

```text
ChatGPT Export
      ↓
Conversation Import
      ↓
SQLite
      ↓
Structured Extraction
      ↓
Pattern Discovery
      ↓
Evidence-Backed Insights
      ↓
Obsidian Knowledge Vault
```

Once that pipeline works reliably, expand toward continuous universal observation.

---

# 42. Guiding Principle

> **Don't optimize for how much Observr knows. Optimize for how much useful, trustworthy knowledge it helps the user discover.**

This principle should guide the design of every feature.

---

# End of Specification