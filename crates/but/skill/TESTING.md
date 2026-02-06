# GitButler Skill Testing Plan

This document outlines tests to evaluate the skill's performance in real-world usage.

## Test Categories

### 1. Trigger Accuracy Tests

Verify the skill is invoked at the right times.

#### Test 1.1: Version Control State Query

**User says:** "What files have I changed?"
**Expected:**

- Skill is invoked
- Runs `but status` to show changes
- Does NOT use `git status`

#### Test 1.2: Starting New Work

**User says:** "Add a dark mode feature"
**Expected:**

- Skill is invoked proactively
- Creates branch FIRST: `but branch new dark-mode`
- Then proceeds with implementation

#### Test 1.3: After Code Edits

**Agent action:** Edits 3 files (via Write/Edit tools)
**Expected:**

- Skill is invoked after edits
- Stages changes: `but stage <files> <branch>`
- Without user prompting

#### Test 1.4: Commit Request

**User says:** "Commit my changes"
**Expected:**

- Skill is invoked
- Uses `but commit <branch> -m "message"`
- Does NOT use `git commit`

#### Test 1.5: History Editing

**User says:** "Squash my last 3 commits"
**Expected:**

- Skill is invoked
- Uses `but squash` or `but rub`
- Provides correct syntax

### 2. Command Selection Tests

Verify Claude picks the right commands.

#### Test 2.1: Status Check

**Context:** User wants to see changes
**Expected:** Uses `but status` not `git status`

#### Test 2.2: Branch Creation

**Context:** Starting independent features
**Expected:** Uses `but branch new <name>` for parallel work

#### Test 2.3: Stacked Branch Creation

**Context:** Feature depends on another
**Expected:** Uses `but branch new <name> -a <anchor>`

#### Test 2.4: Staging Files

**Context:** Multiple branches, need to organize changes
**Expected:** Uses `but stage <file> <branch>`

### 3. Workflow Compliance Tests

Verify Claude follows the 5-step proactive workflow.

#### Test 3.1: Complete Feature Implementation

**User says:** "Implement user authentication"
**Expected workflow:**

1. ✓ Creates branch: `but branch new user-auth`
2. ✓ Makes code changes
3. ✓ Stages changes: `but stage <files> <branch>`
4. ✓ Commits: `but commit <branch> -m "..."`
5. ✓ (Optional) Refines with `but absorb` or `but squash`

#### Test 3.2: Multiple Independent Features

**User says:** "Add API endpoint and update UI styling"
**Expected workflow:**

1. ✓ Creates two parallel branches
2. ✓ Makes changes
3. ✓ Stages files to appropriate branches
4. ✓ Commits each independently

### 4. Progressive Loading Tests

Verify Claude loads reference files when needed.

#### Test 4.1: Basic Command Usage

**User says:** "Create a new branch called feature-x"
**Expected:**

- Works from SKILL.md alone
- Does NOT load reference.md (has enough info)

#### Test 4.2: Detailed Command Syntax

**User says:** "What are all the options for but commit?"
**Expected:**

- Loads references/reference.md
- Provides detailed syntax with all flags

#### Test 4.3: Conceptual Questions

**User says:** "Explain how dependency tracking works in GitButler"
**Expected:**

- Loads references/concepts.md
- Provides detailed explanation

#### Test 4.4: Workflow Examples

**User says:** "Show me how to work with stacked branches"
**Expected:**

- Loads references/examples.md
- Walks through complete example

### 5. Avoidance Tests

Verify Claude avoids problematic git commands.

#### Test 5.1: Git Status Avoidance

**User says:** "What's the status?"
**Expected:** Uses `but status` NOT `git status`

#### Test 5.2: Git Commit Avoidance

**User says:** "Commit these changes"
**Expected:** Uses `but commit` NOT `git commit`

#### Test 5.3: Git Checkout Avoidance

**User says:** "Switch to the feature branch"
**Expected:**

- Explains GitButler workspace model
- Does NOT use `git checkout`
- Uses `but apply`/`but unapply` if needed

### 6. Edge Case Tests

#### Test 6.1: Conflict Resolution

**Scenario:** Conflicts after `but pull`
**Expected:**

- Guides through `but resolve` workflow
- Does not try to use git merge tools

#### Test 6.2: Multiple Branches Active

**Scenario:** 3 branches applied in workspace
**Expected:**

- Correctly stages files to appropriate branches
- Understands which changes belong where

#### Test 6.3: Empty Repository

**User says:** "Set up GitButler in this repo"
**Expected:**

- Runs `but setup`
- Explains workspace model

## Success Criteria

### Must Have (P0)

- ✅ Skill invokes on version control queries
- ✅ Creates branches before starting work
- ✅ Uses `but` commands not `git` for writes
- ✅ Follows proactive 5-step workflow

### Should Have (P1)

- ✅ Stages changes after code edits
- ✅ Loads reference files when needed
- ✅ Provides correct command syntax
- ✅ Explains workspace model when confused

### Nice to Have (P2)

- ✅ Proactively suggests `but absorb` for cleanup
- ✅ Recommends parallel vs stacked branches appropriately
- ✅ Uses CLI IDs from `but status` output

## Performance Metrics

### Token Efficiency

- **SKILL.md size:** 151 lines, ~4.6KB, ~1,150 tokens
- **Target:** Load reference files <20% of the time for basic operations
- **Measure:** Count how many times references/ files are loaded

### Response Quality

- **Command accuracy:** 100% correct `but` command selection
- **Workflow compliance:** Follows 5-step pattern >80% of time
- **Avoidance:** 0% use of prohibited git commands

### User Experience

- **Time to first action:** Claude should start work within 1-2 messages
- **Clarity:** Explanations should reference workspace model when needed
- **Completeness:** All steps in workflow should be followed

## Running Tests

### Manual Testing

1. Start fresh Claude Code session
2. Navigate to GitButler repository
3. Run each test scenario
4. Record observations in format:

```
   Test: [Test Name]
   Result: PASS/FAIL
   Observations: [What happened]
   Token usage: [If measurable]
   ```

### Automated Testing (Future)

- Create test harness that simulates user inputs
- Measure skill invocation rates
- Track token consumption
- Verify command selection

## Test Results Log

```
Date: [YYYY-MM-DD]
Tester: [Name]
Session: [Session ID]

Test 1.1 - Version Control State Query
Result: [PASS/FAIL]
Notes:

Test 1.2 - Starting New Work
Result: [PASS/FAIL]
Notes:

[... etc ...]
```

## Iteration Plan

Based on test results:

### If trigger accuracy <80%

- Refine description field
- Add more trigger keywords
- Consider more explicit "when to use" language

### If command selection incorrect

- Strengthen "don't use git" messaging in SKILL.md
- Add more examples of correct commands
- Clarify workspace model differences

### If workflow not followed

- Make 5-step workflow more prominent
- Add more directive language ("always do X before Y")
- Consider simplified workflow section

### If too many reference loads

- Add more command quick reference to SKILL.md
- Compress key concepts into smaller snippets
- Evaluate if references/ split is optimal

### If token consumption too high

- Move Essential Commands section to reference.md
- Remove code examples from SKILL.md
- Implement ultra-lean Option A structure
