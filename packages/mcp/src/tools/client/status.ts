import { executeGitButlerCommand, hasGitButlerExecutable } from '../../shared/command.js';
import { UnifiedWorktreeChanges } from '../../shared/entities/changes.js';
import {
	BranchCommitsSchema,
	BranchListSchema,
	StackListSchema
} from '../../shared/entities/stacks.js';
import { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

const BaseParamsSchema = z.object({
	project_directory: z.string({ description: 'The absolute path to the project directory' })
});

const StatusParamsSchema = BaseParamsSchema.extend({});

type StatusParams = z.infer<typeof StatusParamsSchema>;

type WorktreeDiffs = {
	filePath: string;
	hunkDiffs: string[];
};

/**
 * Get the file changes of the current GitButler project.
 */
function status(params: StatusParams) {
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
			const hunkDiffs = change.diff.subject.hunks.map((hunk) => hunk.diff);
			result.push({ filePath, hunkDiffs });
		}
	}
	return result;
}

const ListStacksParamsSchema = BaseParamsSchema.extend({});

type ListStacksParams = z.infer<typeof ListStacksParamsSchema>;

/**
 * Get the list of stacks of the current GitButler project.
 */
function listStacks(params: ListStacksParams) {
	const args = ['stacks'];

	return executeGitButlerCommand(params.project_directory, args, StackListSchema);
}

const ListStackBranchesParamsSchema = BaseParamsSchema.extend({
	stack_id: z.string({ description: 'The ID of the stack to list branches from.' })
});

type ListStackBranchesParams = z.infer<typeof ListStackBranchesParamsSchema>;

/**
 * Get the branches of a stack.
 */
function listStackBranches(params: ListStackBranchesParams) {
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

const TOOL_LISTINGS = [
	{
		name: 'get_unified_wortree_changes',
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

	switch (toolName) {
		case 'get_unified_wortree_changes': {
			try {
				const params = StatusParamsSchema.parse(args);
				const result = status(params);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			} catch (error: unknown) {
				if (error instanceof Error)
					return { content: [{ type: 'text', text: `Error: ${error.message}` }], isError: true };

				return { content: [{ type: 'text', text: `Error: ${String(error)}` }], isError: true };
			}
		}
		case 'list_stacks': {
			try {
				const params = ListStacksParamsSchema.parse(args);
				const result = listStacks(params);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			} catch (error: unknown) {
				if (error instanceof Error)
					return { content: [{ type: 'text', text: `Error: ${error.message}` }], isError: true };

				return { content: [{ type: 'text', text: `Error: ${String(error)}` }], isError: true };
			}
		}
		case 'list_stack_branches': {
			try {
				const params = ListStackBranchesParamsSchema.parse(args);
				const result = listStackBranches(params);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			} catch (error: unknown) {
				if (error instanceof Error)
					return { content: [{ type: 'text', text: `Error: ${error.message}` }], isError: true };

				return { content: [{ type: 'text', text: `Error: ${String(error)}` }], isError: true };
			}
		}
		case 'get_branch_commits': {
			try {
				const params = GetBranchCommitsParamsSchema.parse(args);
				const result = getBranchCommits(params);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			} catch (error: unknown) {
				if (error instanceof Error)
					return { content: [{ type: 'text', text: `Error: ${error.message}` }], isError: true };

				return { content: [{ type: 'text', text: `Error: ${String(error)}` }], isError: true };
			}
		}
	}
}
