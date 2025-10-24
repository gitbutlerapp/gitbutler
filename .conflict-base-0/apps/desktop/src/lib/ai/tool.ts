import type { IconName } from '@gitbutler/ui';

type ToolName =
	| 'commit'
	| 'create_branch'
	| 'amend'
	| 'get_project_status'
	| 'create_blank_commit'
	| 'move_file_changes'
	| 'get_commit_details'
	| 'squash_commits'
	| 'split_branch'
	| 'get_branch_changes'
	| 'split_commit';

export type ToolCall = {
	name: ToolName;
	parameters: string;
	result: string;
};

type CommitToolParams = {
	messageTitle: string;
	messageBody: string;
	branchName: string;
	branchDescription: string;
	files: string[];
};

function isCommitToolParams(params: unknown): params is CommitToolParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		typeof (params as CommitToolParams).messageTitle === 'string' &&
		typeof (params as CommitToolParams).messageBody === 'string' &&
		typeof (params as CommitToolParams).branchName === 'string' &&
		typeof (params as CommitToolParams).branchDescription === 'string' &&
		Array.isArray((params as CommitToolParams).files) &&
		(params as CommitToolParams).files.every((file) => typeof file === 'string')
	);
}

type CreateBranchToolParams = {
	branchName: string;
	branchDescription: string;
};

function isCreateBranchToolParams(params: unknown): params is CreateBranchToolParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		typeof (params as CreateBranchToolParams).branchName === 'string' &&
		typeof (params as CreateBranchToolParams).branchDescription === 'string'
	);
}

type AmendToolParams = {
	commitId: string;
	messageTitle: string;
	messageBody: string;
	stackId: string;
	files: string[];
};

function isAmendToolParams(params: unknown): params is AmendToolParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		typeof (params as AmendToolParams).commitId === 'string' &&
		typeof (params as AmendToolParams).messageTitle === 'string' &&
		typeof (params as AmendToolParams).messageBody === 'string' &&
		typeof (params as AmendToolParams).stackId === 'string' &&
		Array.isArray((params as AmendToolParams).files) &&
		(params as AmendToolParams).files.every((file) => typeof file === 'string')
	);
}

type GetProjectStatusToolParams = {
	filterChanges: string[] | null;
};

function isGetProjectStatusToolParams(params: unknown): params is GetProjectStatusToolParams {
	return (
		(typeof params === 'object' &&
			params !== null &&
			Array.isArray((params as GetProjectStatusToolParams).filterChanges) &&
			(params as GetProjectStatusToolParams).filterChanges!.every(
				(change) => typeof change === 'string'
			)) ||
		(params as GetProjectStatusToolParams).filterChanges === null
	);
}

type CreateBlankCommitToolParams = {
	messageTitle: string;
	messageBody: string;
	stackId: string;
	parentId: string;
};

function isCreateBlankCommitToolParams(params: unknown): params is CreateBlankCommitToolParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		typeof (params as CreateBlankCommitToolParams).messageTitle === 'string' &&
		typeof (params as CreateBlankCommitToolParams).messageBody === 'string' &&
		typeof (params as CreateBlankCommitToolParams).stackId === 'string' &&
		typeof (params as CreateBlankCommitToolParams).parentId === 'string'
	);
}

type MoveFileChangesToolParams = {
	sourceCommitId: string;
	sourceStackId: string;
	destinationCommitId: string;
	destinationStackId: string;
	files: string[];
};

function isMoveFileChangesToolParams(params: unknown): params is MoveFileChangesToolParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		typeof (params as MoveFileChangesToolParams).sourceCommitId === 'string' &&
		typeof (params as MoveFileChangesToolParams).sourceStackId === 'string' &&
		typeof (params as MoveFileChangesToolParams).destinationCommitId === 'string' &&
		typeof (params as MoveFileChangesToolParams).destinationStackId === 'string' &&
		Array.isArray((params as MoveFileChangesToolParams).files) &&
		(params as MoveFileChangesToolParams).files.every((file) => typeof file === 'string')
	);
}

type GetCommitDetailsToolParams = {
	commitId: string;
};

function isGetCommitDetailsToolParams(params: unknown): params is GetCommitDetailsToolParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		typeof (params as GetCommitDetailsToolParams).commitId === 'string'
	);
}

type SquashCommitsToolParams = {
	stackId: string;
	sourceCommitIds: string[];
	destinationCommitId: string;
	messageTitle: string;
	messageBody: string;
};

function isSquashCommitsToolParams(params: unknown): params is SquashCommitsToolParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		typeof (params as SquashCommitsToolParams).stackId === 'string' &&
		Array.isArray((params as SquashCommitsToolParams).sourceCommitIds) &&
		(params as SquashCommitsToolParams).sourceCommitIds.every(
			(commitId) => typeof commitId === 'string'
		) &&
		typeof (params as SquashCommitsToolParams).destinationCommitId === 'string' &&
		typeof (params as SquashCommitsToolParams).messageTitle === 'string' &&
		typeof (params as SquashCommitsToolParams).messageBody === 'string'
	);
}

type SplitBranchToolParams = {
	sourceBranchName: string;
	newBranchName: string;
	filesToSplitOff: string[];
};

function isSplitBranchToolParams(params: unknown): params is SplitBranchToolParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		typeof (params as SplitBranchToolParams).sourceBranchName === 'string' &&
		typeof (params as SplitBranchToolParams).newBranchName === 'string' &&
		Array.isArray((params as SplitBranchToolParams).filesToSplitOff) &&
		(params as SplitBranchToolParams).filesToSplitOff.every((file) => typeof file === 'string')
	);
}

type GetBranchChangesParameters = {
	branchName: string;
};

function isGetBranchChangesParameters(params: unknown): params is GetBranchChangesParameters {
	return (
		typeof params === 'object' &&
		params !== null &&
		typeof (params as GetBranchChangesParameters).branchName === 'string'
	);
}

type CommitShard = {
	messageTitle: string;
	messageBody: string;
	files: string[];
};

function isCommitShard(shard: unknown): shard is CommitShard {
	return (
		typeof shard === 'object' &&
		shard !== null &&
		typeof (shard as CommitShard).messageTitle === 'string' &&
		typeof (shard as CommitShard).messageBody === 'string' &&
		Array.isArray((shard as CommitShard).files) &&
		(shard as CommitShard).files.every((file) => typeof file === 'string')
	);
}

type SplitCommitParameters = {
	sourceStackId: string;
	sourceCommitId: string;
	shards: CommitShard[];
};

function isSplitCommitParameters(params: unknown): params is SplitCommitParameters {
	return (
		typeof params === 'object' &&
		params !== null &&
		typeof (params as SplitCommitParameters).sourceStackId === 'string' &&
		typeof (params as SplitCommitParameters).sourceCommitId === 'string' &&
		Array.isArray((params as SplitCommitParameters).shards) &&
		(params as SplitCommitParameters).shards.every(isCommitShard)
	);
}

interface BaseParsedToolCall {
	name: ToolName;
	parameters:
		| CommitToolParams
		| CreateBranchToolParams
		| AmendToolParams
		| GetProjectStatusToolParams
		| CreateBlankCommitToolParams
		| MoveFileChangesToolParams
		| GetCommitDetailsToolParams
		| SquashCommitsToolParams
		| SplitBranchToolParams
		| GetBranchChangesParameters
		| SplitCommitParameters
		| undefined;
	result: string;
	isError: boolean;
	rawParameters: unknown;
	rawResult: unknown;
}

interface ParsedCommitToolCall extends BaseParsedToolCall {
	name: 'commit';
	parameters: CommitToolParams | undefined;
	parsedResult: CommitToolResult | undefined;
}

interface ParsedCreateBranchToolCall extends BaseParsedToolCall {
	name: 'create_branch';
	parameters: CreateBranchToolParams | undefined;
}

interface ParsedAmendToolCall extends BaseParsedToolCall {
	name: 'amend';
	parameters: AmendToolParams | undefined;
	parsedResult: CommitToolResult | undefined;
}

interface ParsedGetProjectStatusToolCall extends BaseParsedToolCall {
	name: 'get_project_status';
	parameters: GetProjectStatusToolParams | undefined;
}

interface ParsedCreateBlankCommitToolCall extends BaseParsedToolCall {
	name: 'create_blank_commit';
	parameters: CreateBlankCommitToolParams | undefined;
}

interface ParsedMoveFileChangesToolCall extends BaseParsedToolCall {
	name: 'move_file_changes';
	parameters: MoveFileChangesToolParams | undefined;
}

interface ParsedGetCommitDetailsToolCall extends BaseParsedToolCall {
	name: 'get_commit_details';
	parameters: GetCommitDetailsToolParams | undefined;
}

interface ParsedSquashCommitsToolCall extends BaseParsedToolCall {
	name: 'squash_commits';
	parameters: SquashCommitsToolParams | undefined;
}

interface ParsedSplitBranchToolCall extends BaseParsedToolCall {
	name: 'split_branch';
	parameters: SplitBranchToolParams | undefined;
}

interface ParsedGetBranchChangesToolCall extends BaseParsedToolCall {
	name: 'get_branch_changes';
	parameters: GetBranchChangesParameters | undefined;
}

interface ParsedSplitCommitToolCall extends BaseParsedToolCall {
	name: 'split_commit';
	parameters: SplitCommitParameters | undefined;
}

export type ParsedToolCall =
	| ParsedCommitToolCall
	| ParsedCreateBranchToolCall
	| ParsedAmendToolCall
	| ParsedGetProjectStatusToolCall
	| ParsedCreateBlankCommitToolCall
	| ParsedMoveFileChangesToolCall
	| ParsedGetCommitDetailsToolCall
	| ParsedSquashCommitsToolCall
	| ParsedSplitBranchToolCall
	| ParsedGetBranchChangesToolCall
	| ParsedSplitCommitToolCall;

function safeParseJson(jsonString: string): unknown {
	try {
		return JSON.parse(jsonString);
	} catch (error) {
		console.error('Failed to parse JSON:', error);
		return undefined;
	}
}

function isErrorQuery(something: unknown): boolean {
	return (
		typeof something === 'object' &&
		something !== null &&
		'error' in something &&
		(something as { error: unknown }).error !== undefined
	);
}

type CommitToolResult = {
	result: {
		newCommit: string | null;
		pathsToRejectedChanges: [string, string][];
	};
};

function isCommitToolQuery(result: unknown): result is CommitToolResult {
	return (
		typeof result === 'object' &&
		result !== null &&
		typeof (result as CommitToolResult).result === 'object' &&
		(result as CommitToolResult).result !== null &&
		(typeof (result as CommitToolResult).result.newCommit === 'string' ||
			(result as CommitToolResult).result.newCommit === null) &&
		Array.isArray((result as CommitToolResult).result.pathsToRejectedChanges) &&
		(result as CommitToolResult).result.pathsToRejectedChanges.every(
			(item) =>
				Array.isArray(item) &&
				item.length === 2 &&
				item.every((subItem) => typeof subItem === 'string')
		)
	);
}

export function getToolCallIcon(name: ToolName, isError: boolean): IconName {
	if (isError) {
		return 'error';
	}

	switch (name) {
		case 'split_commit':
			return 'branch-shadow-commit';
		case 'commit':
			return 'commit';
		case 'amend':
			return 'amend-commit';
		case 'create_blank_commit':
			return 'blank-commit';
		case 'create_branch':
			return 'branch-local';
		case 'get_project_status':
			return 'info';
		case 'move_file_changes':
			return 'move-commit-file-small';
		case 'get_commit_details':
			return 'info';
		case 'squash_commits':
			return 'squash-commit';
		case 'split_branch':
			return 'branch-local';
		case 'get_branch_changes':
			return 'info';
	}
}

export function parseToolCall(toolCall: ToolCall): ParsedToolCall {
	const rawParams = safeParseJson(toolCall.parameters);
	const rawResult = safeParseJson(toolCall.result);
	const isError = isErrorQuery(rawResult);

	switch (toolCall.name) {
		case 'commit': {
			const parameters = isCommitToolParams(rawParams) ? rawParams : undefined;
			const parsedResult = isCommitToolQuery(rawResult) ? rawResult : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				parsedResult,
				isError,
				rawParameters: rawParams,
				rawResult
			};
		}
		case 'create_branch': {
			const parameters = isCreateBranchToolParams(rawParams) ? rawParams : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				isError,
				rawParameters: rawParams,
				rawResult
			};
		}
		case 'amend': {
			const parameters = isAmendToolParams(rawParams) ? rawParams : undefined;
			const parsedResult = isCommitToolQuery(rawResult) ? rawResult : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				isError,
				rawParameters: rawParams,
				rawResult,
				parsedResult
			};
		}
		case 'get_project_status': {
			const parameters = isGetProjectStatusToolParams(rawParams) ? rawParams : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				isError,
				rawParameters: rawParams,
				rawResult
			};
		}
		case 'create_blank_commit': {
			const parameters = isCreateBlankCommitToolParams(rawParams) ? rawParams : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				isError,
				rawParameters: rawParams,
				rawResult
			};
		}
		case 'move_file_changes': {
			const parameters = isMoveFileChangesToolParams(rawParams) ? rawParams : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				isError,
				rawParameters: rawParams,
				rawResult
			};
		}
		case 'get_commit_details': {
			const parameters = isGetCommitDetailsToolParams(rawParams) ? rawParams : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				isError,
				rawParameters: rawParams,
				rawResult
			};
		}
		case 'squash_commits': {
			const parameters = isSquashCommitsToolParams(rawParams) ? rawParams : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				isError,
				rawParameters: rawParams,
				rawResult
			};
		}
		case 'split_branch': {
			const parameters = isSplitBranchToolParams(rawParams) ? rawParams : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				isError,
				rawParameters: rawParams,
				rawResult
			};
		}
		case 'get_branch_changes': {
			const parameters = isGetBranchChangesParameters(rawParams) ? rawParams : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				isError,
				rawParameters: rawParams,
				rawResult
			};
		}
		case 'split_commit': {
			const parameters = isSplitCommitParameters(rawParams) ? rawParams : undefined;
			return {
				name: toolCall.name,
				parameters,
				result: toolCall.result,
				isError,
				rawParameters: rawParams,
				rawResult
			};
		}
	}
}
