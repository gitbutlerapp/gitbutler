import type { BrandedId } from '@gitbutler/shared/utils/branding';

export type WorkspaceRuleId = BrandedId<'WorkspaceRule'>;
/**
 * A workspace rule.
 * @remarks
 * A rule is evaluated in the app and determines what happens to files or changes based on triggers, filters, and actions.
 *
 * Multiple rules can defined and will be evaluated in the order they are defined using an OR logic.
 */
export interface WorkspaceRule {
	/** A UUID unique identifier for the rule. */
	id: WorkspaceRuleId;
	/** The time when the rule was created, represented as a Unix timestamp in milliseconds. */
	createdAt: string; // ISO string or Date, depending on backend serialization
	/** Whether the rule is currently enabled or not. */
	enabled: boolean;
	/** The trigger of the rule is what causes it to be evaluated in the app. */
	trigger: Trigger;
	/** These filters determine what files or changes the rule applies to
	 *  Multiple filters are combined with AND logic (i.e. all conditions must be met).
	 *  This allows for the expressions of rules like "If a file is modified, its path matches
	 *  the regex 'src/.*', and its content matches the regex 'TODO', then do something.
	 *  */
	filters: Filter[];
	/** The action determines what happens to the files or changes that matched the filters. */
	action: Action;
}

/**
 * Represents the kinds of events in the app that can cause a rule to be evaluated.
 */
export type Trigger =
	/** When a file is added, removed or modified in the Git worktree. */
	'fileSytemChange';

/**
 * A filter is a condition that determines what files or changes the rule applies to.
 * Multiple conditions in a filter are combined with AND logic.
 */
export type Filter =
	| { type: 'pathMatchesRegex'; subject: string } // regex patterns as strings
	| { type: 'contentMatchesRegex'; subject: string } // regex patterns as strings
	| { type: 'fileChangeType'; subject: TreeStatus }
	| { type: 'semanticType'; subject: SemanticType };

/**
 * Represents the type of change that occurred in the Git worktree.
 * Matches the TreeStatus of the TreeChange.
 */
export type TreeStatus = 'addition' | 'deletion' | 'modification' | 'rename';

/**
 * Represents a semantic type of change that was inferred for the change.
 * Typically this means a heuristic or an LLM determined that a change represents a refactor, a new feature, a bug fix, or documentation update.
 */
export type SemanticType =
	| { type: 'refactor' }
	| { type: 'newFeature' }
	| { type: 'bugFix' }
	| { type: 'documentation' }
	| { type: 'userDefined'; subject: string };

/**
 * Represents an action that can be taken based on the rule evaluation.
 * An action can be either explicit (user defined) or implicit (determined by heuristics or AI).
 */
export type Action =
	| { type: 'explicit'; subject: Operation }
	| { type: 'implicit'; subject: ImplicitOperation };

/**
 * Represents the operation that a user can configure to be performed in an explicit action.
 */
export type Operation =
	| { type: 'assign'; stackId: string }
	| { type: 'amend'; commitId: string }
	| { type: 'newCommit'; branchName: string };

/**
 * Represents the implicit operation that is determined by heuristics or AI.
 */
export type ImplicitOperation =
	| { type: 'assignToAppropriateBranch' }
	| { type: 'absorbIntoDependentCommit' }
	| { type: 'llmPrompt'; subject: string };

/**
 * A request to create a new workspace rule.
 */
export interface CreateRuleRequest {
	/** The trigger that causes the rule to be evaluated. */
	trigger: Trigger;
	/** The filters that determine what files or changes the rule applies to. Cannot be empty. */
	filters: Filter[];
	/** The action that determines what happens to the files or changes that matched the filters. */
	action: Action;
}

/**
 * A request to update an existing workspace rule.
 */
export interface UpdateRuleRequest {
	/** The ID of the rule to update. */
	id: WorkspaceRuleId;
	/** The new enabled state of the rule. If not provided, the existing state is retained. */
	enabled: boolean | null;
	/** The new trigger for the rule. If not provided, the existing trigger is retained. */
	trigger: Trigger | null;
	/** The new filters for the rule. If not provided, the existing filters are retained. */
	filters: Filter[] | null;
	/** The new action for the rule. If not provided, the existing action is retained. */
	action: Action | null;
}
