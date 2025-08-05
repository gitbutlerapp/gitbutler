import type { BrandedId } from '@gitbutler/shared/utils/branding';
import type { FileStatus } from '@gitbutler/ui/components/file/types';

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
	filters: RuleFilter[];
	/** The action determines what happens to the files or changes that matched the filters. */
	action: RuleAction;
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
export type RuleFilter =
	| { type: 'pathMatchesRegex'; subject: string } // regex patterns as strings
	| { type: 'contentMatchesRegex'; subject: string } // regex patterns as strings
	| { type: 'fileChangeType'; subject: FileStatus }
	| { type: 'semanticType'; subject: SemanticTypeFilter };

export type RuleFilterType = RuleFilter['type'];
export const RULE_FILTER_TYPES = [
	'pathMatchesRegex',
	'contentMatchesRegex',
	'fileChangeType',
	'semanticType'
] satisfies RuleFilterType[];

export type RuleFilterMap = {
	[K in RuleFilterType]: RuleFilterSubject<K> | null;
};

export function canAddMoreFilters(filters: RuleFilterType[]): boolean {
	return filters.length < RULE_FILTER_TYPES.length;
}

type FilterCountMap = {
	[K in RuleFilterType as `${K}Count`]: number;
};

export function getFilterCountMap(rules: WorkspaceRule[]): FilterCountMap {
	const countMap: FilterCountMap = {
		pathMatchesRegexCount: 0,
		contentMatchesRegexCount: 0,
		fileChangeTypeCount: 0,
		semanticTypeCount: 0
	};

	for (const rule of rules) {
		const visitedFilters = new Set<RuleFilterType>();
		for (const filter of rule.filters) {
			if (filter.type in countMap) {
				if (!visitedFilters.has(filter.type)) {
					visitedFilters.add(filter.type);
					// Increment the count for this filter type
					countMap[`${filter.type}Count`] += 1;
				}
			}
		}
	}

	return countMap;
}

export type RuleFilterSubject<T extends RuleFilterType> = Extract<
	RuleFilter,
	{ type: T }
>['subject'];

/**
 * Represents the type of change that occurred in the Git worktree.
 * Now using FileStatus directly for consistency across the codebase.
 */
export type TreeStatus = FileStatus;

export function treeStatusToString(status: FileStatus): string {
	switch (status) {
		case 'addition':
			return 'Addition';
		case 'deletion':
			return 'Deletion';
		case 'modification':
			return 'Modification';
		case 'rename':
			return 'Rename';
	}
}

export function treeStatusToShortString(status: FileStatus): string {
	switch (status) {
		case 'addition':
			return 'added';
		case 'deletion':
			return 'deleted';
		case 'modification':
			return 'modified';
		case 'rename':
			return 'renamed';
	}
}

/**
 * Represents a semantic type of change that was inferred for the change.
 * Typically this means a heuristic or an LLM determined that a change represents a refactor, a new feature, a bug fix, or documentation update.
 */
export type SemanticTypeFilter =
	| { type: 'refactor' }
	| { type: 'newFeature' }
	| { type: 'bugFix' }
	| { type: 'documentation' }
	| { type: 'userDefined'; subject: string };

export type SemanticType = SemanticTypeFilter['type'];

export function semanticTypeToString(semanticType: SemanticType): string {
	switch (semanticType) {
		case 'refactor':
			return 'Refactor 🔧';
		case 'newFeature':
			return 'New Feature ✨';
		case 'bugFix':
			return 'Bug Fix 🐛';
		case 'documentation':
			return 'Documentation 📚';
		default:
			return semanticType; // For user-defined types, return the subject directly
	}
}

/**
 * TODO: Add the user defined semantic type to the list of semantic types.
 * It's not currently used in the application, but it might be added later.
 */
export const SEMANTIC_TYPES = [
	'refactor',
	'newFeature',
	'bugFix',
	'documentation'
] satisfies SemanticType[];

/**
 * Represents an action that can be taken based on the rule evaluation.
 * An action can be either explicit (user defined) or implicit (determined by heuristics or AI).
 */
export type RuleAction =
	| { type: 'explicit'; subject: Operation }
	| { type: 'implicit'; subject: ImplicitOperation };

/**
 * Represents the operation that a user can configure to be performed in an explicit action.
 */
export type Operation =
	| { type: 'assign'; subject: { target: StackTarget } }
	| { type: 'amend'; subject: { commit_id: string } }
	| { type: 'newCommit'; subject: { branch_name: string } };

/**
 * The target stack for a given operation. It's either specifying a specific stack ID, or alternaitvely the leftmost or rightmost stack in the workspace.
 */
type StackIdTarget = {
	type: 'stackId';
	subject: string;
};

type LeftmostTarget = { type: 'leftmost' };
type RightmostTarget = { type: 'rightmost' };
export type StackTarget = StackIdTarget | LeftmostTarget | RightmostTarget;

type StackTargetType = StackTarget['type'];

type StackTargetTypeCount = {
	[K in StackTargetType as `assignmentTargetCount-${K}`]: number;
};

export function getStackTargetTypeCountMap(rules: WorkspaceRule[]): StackTargetTypeCount {
	const countMap: StackTargetTypeCount = {
		'assignmentTargetCount-stackId': 0,
		'assignmentTargetCount-leftmost': 0,
		'assignmentTargetCount-rightmost': 0
	};

	for (const rule of rules) {
		if (rule.action.type === 'explicit' && rule.action.subject.type === 'assign') {
			const target = rule.action.subject.subject.target;
			countMap[`assignmentTargetCount-${target.type}`] += 1;
		}
	}

	return countMap;
}

const UNIT_SEP = '\u001F';

export function encodeStackTarget(stackTarget: StackTarget): string {
	switch (stackTarget.type) {
		case 'stackId':
			return `${stackTarget.type}${UNIT_SEP}${stackTarget.subject}`;
		case 'leftmost':
			return 'leftmost';
		case 'rightmost':
			return 'rightmost';
	}
}

export function decodeStackTarget(encoded: string): StackTarget {
	if (encoded === 'leftmost') {
		return { type: 'leftmost' };
	}

	if (encoded === 'rightmost') {
		return { type: 'rightmost' };
	}

	const [type, subject] = encoded.split(UNIT_SEP);

	if (type === 'stackId' && subject) {
		return { type: 'stackId', subject };
	}

	throw new Error(`Unknown stack target type: ${type}`);
}

export function compareStackTarget(encoded: string, target: StackTarget | undefined): boolean {
	if (!target) return false;
	const encodedTarget = encodeStackTarget(target);
	return encoded === encodedTarget;
}

export function isStackIdTarget(target: StackTarget | string): boolean {
	const decoded = typeof target === 'string' ? decodeStackTarget(target) : target;
	return decoded.type === 'stackId';
}

export function isLeftmostTarget(target: StackTarget | string): boolean {
	const decoded = typeof target === 'string' ? decodeStackTarget(target) : target;
	return decoded.type === 'leftmost';
}

export function isRightmostTarget(target: StackTarget | string): boolean {
	const decoded = typeof target === 'string' ? decodeStackTarget(target) : target;
	return decoded.type === 'rightmost';
}

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
	/** The filters that determine what files or changes the rule applies to. If empty, all files are matched. */
	filters: RuleFilter[];
	/** The action that determines what happens to the files or changes that matched the filters. */
	action: RuleAction;
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
	filters: RuleFilter[] | null;
	/** The new action for the rule. If not provided, the existing action is retained. */
	action: RuleAction | null;
}
