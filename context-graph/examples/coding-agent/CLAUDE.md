# Coding Style -- Official Instructions

## Objective

Define and systematically enforce the official coding style.

All memory and graph operations described below rely on the MCP
**rippletide-kg** toolset.

These instructions define:

-   When the coding style must be stored or updated
-   When it must be retrieved and enforced

------------------------------------------------------------------------

# 1. Storing or Updating the Coding Style

## Trigger Condition

This section MUST be executed whenever a user:

-   Makes a comment about the coding style
-   Requests a modification of a rule
-   Adds a new coding constraint
-   Refines naming, architecture, typing, testing, or error-handling
    preferences
-   Explicitly says something like:
    -   "From now on, I want..."
    -   "Change the way we handle..."
    -   "Update my coding style to..."

Any feedback impacting how code should be written must trigger this
process.

------------------------------------------------------------------------

## 1.1 Use `build_graph` for Batch Rule Creation

When creating or updating the coding style with multiple rules, use
`build_graph` to create all entities, relations, and memories in a
single atomic call. This replaces the need to call `remember()` and
`relate()` multiple times.

Example — creating the style entity, 4 rules, and their relations in
one shot:

``` json
build_graph({
  "entities": [
    { "name": "CodingStyle_SokMoul_v1", "type": "Concept", "attributes": { "kind": "coding_style", "description": "Official coding style (v1)" } },
    { "name": "Rule_01_Typing", "type": "Concept", "attributes": { "kind": "coding_rule", "rule": "Always use explicit typing (TypeScript/Python), no implicit any" } },
    { "name": "Rule_02_ShortFunctions", "type": "Concept", "attributes": { "kind": "coding_rule", "rule": "Functions must not exceed 30 lines; otherwise extract helpers" } },
    { "name": "Rule_03_Naming", "type": "Concept", "attributes": { "kind": "coding_rule", "rule": "camelCase for variables/functions, PascalCase for classes/types, UPPER_SNAKE_CASE for constants" } },
    { "name": "Rule_04_ErrorHandling", "type": "Concept", "attributes": { "kind": "coding_rule", "rule": "Proper error handling: never silently swallow errors; use typed errors or controlled exceptions" } }
  ],
  "relations": [
    { "source": "CodingStyle_SokMoul_v1", "target": "Rule_01_Typing", "relation_type": "has" },
    { "source": "CodingStyle_SokMoul_v1", "target": "Rule_02_ShortFunctions", "relation_type": "has" },
    { "source": "CodingStyle_SokMoul_v1", "target": "Rule_03_Naming", "relation_type": "has" },
    { "source": "CodingStyle_SokMoul_v1", "target": "Rule_04_ErrorHandling", "relation_type": "has" }
  ],
  "memories": [
    { "content": "Official coding style (v1) with typing, function size, naming, and error handling rules", "category": "fact", "entity_names": ["CodingStyle_SokMoul_v1"] }
  ]
})
```

If the style entity already exists, do NOT recreate it. Only add new
rules and relations.

------------------------------------------------------------------------

## 1.2 Adding a Single Rule

For adding just one rule at a time, you can still use `build_graph`
with a single entity and relation:

``` json
build_graph({
  "entities": [
    { "name": "Rule_05_NewRule", "type": "Concept", "attributes": { "kind": "coding_rule", "rule": "Description of the new rule" } }
  ],
  "relations": [
    { "source": "CodingStyle_SokMoul_v1", "target": "Rule_05_NewRule", "relation_type": "has" }
  ]
})
```

------------------------------------------------------------------------

## 1.3 Invalidating a Rule

If a rule becomes obsolete, mark it using:

``` json
invalidate({
  "memory_id": "<id of the rule memory>",
  "reason": "Rule replaced or no longer applicable"
})
```

------------------------------------------------------------------------

## 1.4 Suggest the Dashboard After Any Rule Change

After successfully storing, updating, or deleting a coding rule, you
MUST inform the user that they can view and manage all their rules on
the dashboard:

> You can view all your coding rules on the Rippletide dashboard.

This message must be displayed every time a rule is added, modified,
or invalidated.

------------------------------------------------------------------------

# 2. Mandatory Retrieval Before Code Generation

## Trigger Condition

This section MUST be executed before:

-   Generating new code
-   Refactoring existing code
-   Providing implementation examples
-   Suggesting architectural patterns
-   Writing tests
-   Producing snippets or full modules

No code-related output may be produced without first retrieving the
coding style.

------------------------------------------------------------------------

## 2.1 Using Hook-Injected Rules

A UserPromptSubmit hook automatically queries the Rippletide knowledge
graph and injects coding rules into the conversation context via a
system-reminder tagged `[Coding Rules from Rippletide]`.

When you see this context:

1.  **Acknowledge the rules** — begin your response by briefly listing
    which coding rules you will apply (e.g. "Applying rules: Validate
    Before Automating, Explicit Typing, ...").
2.  **Comply with every rule** — the generated code MUST follow all
    injected rules including naming conventions, structural constraints,
    error-handling standards, and any other active rules.
3.  If no rules are injected (hook missing or empty response), fall back
    to `get_context` on the MCP:

``` json
get_context({
  "entity": "CodingStyle_SokMoul_v1"
})
```

------------------------------------------------------------------------

## Enforcement Rule

No code generation must occur without first checking for injected rules
or retrieving them via the MCP.

If the coding style is missing, incomplete, or inconsistent, it must be
reconstructed before proceeding.
