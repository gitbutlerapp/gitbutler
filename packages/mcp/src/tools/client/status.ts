import { BaseParamsSchema } from './shared.js';
import { executeGitButlerCommand, hasGitButlerExecutable } from '../../shared/command.js';
import { DiffHunk, UnifiedWorktreeChanges } from '../../shared/entities/changes.js';
import { HunkDependenciesSchema } from '../../shared/entities/dependencies.js';
import {
	BranchCommitsSchema,
	BranchListSchema,
	StackListSchema
} from '../../shared/entities/stacks.js';
import { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

const StatusParamsSchema = BaseParamsSchema.extend({});

type StatusParams = z.infer<typeof StatusParamsSchema>;

type WorktreeDiffs = {
	filePath: string;
	hunks: DiffHunk[];
};

/**
 * Get the file changes of the current GitButler project.
 */
export function status(params: StatusParams) {
	const args = ['status', '--unified-diff'];

	const unifiedWorktreeChanges = executeGitButlerCommand(
		params.project_directory,
		args,
		UnifiedWorktreeChanges
	);

	const result: WorktreeDiffs[] = [];
	for (const change of unifiedWorktreeChanges.changes) {
		if (change.diff.type === 'Patch') {
			const filePath = change.treeChange.path;
			const hunks = change.diff.subject.hunks;
			result.push({ filePath, hunks });
		}
	}
	return result;
}

const ListStacksParamsSchema = BaseParamsSchema.extend({});

type ListStacksParams = z.infer<typeof ListStacksParamsSchema>;

type ExtendedStackHead = {
	name: string;
	tip: string;
	archived: boolean;
	description: string | null;
};

type ExtendedStackListing = {
	id: string;
	heads: ExtendedStackHead[];
};

/**
 * Get the list of stacks of the current GitButler project.
 */
export function listStacks(params: ListStacksParams): ExtendedStackListing[] {
	const args = ['stacks', '-w'];

	const stacks = executeGitButlerCommand(params.project_directory, args, StackListSchema);
	const result: ExtendedStackListing[] = [];
	for (const stack of stacks) {
		const stackBranches = listStackBranches({
			project_directory: params.project_directory,
			stack_id: stack.id
		});

		result.push({
			id: stack.id,
			heads: stackBranches.map((branch) => ({
				name: branch.name,
				tip: branch.tip,
				archived: branch.archived,
				description: branch.description
			}))
		});
	}

	return result;
}

const ListStackBranchesParamsSchema = BaseParamsSchema.extend({
	stack_id: z.string({ description: 'The ID of the stack to list branches from.' })
});

type ListStackBranchesParams = z.infer<typeof ListStackBranchesParamsSchema>;

/**
 * Get the branches of a stack.
 */
export function listStackBranches(params: ListStackBranchesParams) {
	const args = ['stack-branches', params.stack_id];

	return executeGitButlerCommand(params.project_directory, args, BranchListSchema);
}

const GetBranchCommitsParamsSchema = BaseParamsSchema.extend({
	stack_id: z.string({ description: 'The ID of the stack to get commits from.' }),
	branch_name: z.string({ description: 'The name of the branch to get commits from.' })
});

type GetBranchCommitsParams = z.infer<typeof GetBranchCommitsParamsSchema>;

/**
 * Get the commits of a branch.
 */
function getBranchCommits(params: GetBranchCommitsParams) {
	const args = ['stack-branch-commits', params.stack_id, params.branch_name];

	return executeGitButlerCommand(params.project_directory, args, BranchCommitsSchema);
}

type GetHunkDependenciesParams = z.infer<typeof BaseParamsSchema>;

type FileStackDependencies = {
	/**
	 * The file path of the diff.
	 */
	filePath: string;

	/**
	 * The commit IDs that that the file is locked to.
	 */
	lockedToCommitIds: string[];
};

function getHunkDependencies(params: GetHunkDependenciesParams) {
	const args = ['dep', '--simple'];

	const dependencies = executeGitButlerCommand(
		params.project_directory,
		args,
		HunkDependenciesSchema
	);

	const fileDependencies = new Map<string, Set<string>>();

	for (const [filePath, _, locks] of dependencies.diffs) {
		if (!fileDependencies.has(filePath)) {
			fileDependencies.set(filePath, new Set());
		}
		const fileLocks = fileDependencies.get(filePath);
		if (fileLocks) {
			for (const lock of locks) {
				fileLocks.add(lock.commitId);
			}
		}
	}

	// Serialize
	const serializedDependencies: FileStackDependencies[] = Array.from(
		fileDependencies.entries()
	).map(([filePath, locks]) => ({
		filePath,
		lockedToCommitIds: Array.from(locks)
	}));

	return serializedDependencies;
}

const TOOL_LISTINGS = [
	{
		name: 'list_file_changes',
		description:
			'Get the file changes of the current GitButler project. Always call this tool when you want to get the file changes.',
		inputSchema: zodToJsonSchema(StatusParamsSchema)
	},
	{
		name: 'list_stacks',
		description:
			'Get the list of stacks of the current GitButler project. This returns a list of the stack IDs and their branch names.',
		inputSchema: zodToJsonSchema(ListStacksParamsSchema)
	},
	{
		name: 'list_stack_branches',
		description:
			'Get the branches of a stack. This returns a list of branch information, including whether they are archived or not.',
		inputSchema: zodToJsonSchema(ListStackBranchesParamsSchema)
	},
	{
		name: 'get_branch_commits',
		description:
			'Get the commits of a branch in a stack. This returns a list of commit information, including authors, message and whether they are conflicted.',
		inputSchema: zodToJsonSchema(GetBranchCommitsParamsSchema)
	},
	{
		name: 'get_hunk_dependencies',
		description:
			'Get the dependencies of a hunk in a stack. This returns a list of hunk dependencies, including the stack ID and commit ID.',
		inputSchema: zodToJsonSchema(BaseParamsSchema)
	}
] as const;

type ToolName = (typeof TOOL_LISTINGS)[number]['name'];

function isToolName(name: string): name is ToolName {
	return TOOL_LISTINGS.some((tool) => tool.name === name);
}

export function getStatusToolListing() {
	if (!hasGitButlerExecutable()) {
		return [];
	}
	return TOOL_LISTINGS;
}

export async function getStatusToolRequestHandler(
	toolName: string,
	args: Record<string, unknown>
): Promise<CallToolResult | null> {
	if (!isToolName(toolName) || !hasGitButlerExecutable()) {
		return null;
	}

	try {
		switch (toolName) {
			case 'list_file_changes': {
				const params = StatusParamsSchema.parse(args);
				const result = status(params);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'list_stacks': {
				const params = ListStacksParamsSchema.parse(args);
				const result = listStacks(params);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'list_stack_branches': {
				const params = ListStackBranchesParamsSchema.parse(args);
				const result = listStackBranches(params);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'get_branch_commits': {
				const params = GetBranchCommitsParamsSchema.parse(args);
				const result = getBranchCommits(params);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'get_hunk_dependencies': {
				const params = BaseParamsSchema.parse(args);
				const result = getHunkDependencies(params);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
		}
	} catch (error: unknown) {
		if (error instanceof z.ZodError) {
			throw error;
		}

		if (error instanceof Error) {
			return { content: [{ type: 'text', text: error.message }] };
		}

		return { content: [{ type: 'text', text: String(error) }] };
	}
}
