import { BaseParamsSchema, DiffSpec, getBranchRef } from './shared.js';
import { listStackBranches, listStacks } from './status.js';
import { executeGitButlerCommand, hasGitButlerExecutable } from '../../shared/command.js';
import { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

const CommitParamsSchema = BaseParamsSchema.extend({
	message: z.string({ description: 'The commit message' }),
	all: z.boolean().optional().default(false),
	filePaths: z.array(z.string()).optional().default([]),
	branch: z.string({ description: 'The branch to commit to' })
});

type CommitParams = z.infer<typeof CommitParamsSchema>;

/**
 * Commit changes.
 */
function commit(params: CommitParams) {
	const args = ['commit', '--message', params.message];

	if (params.all) {
		if (params.filePaths.length > 0) {
			throw new Error('Cannot use --all and file paths together');
		}
	}

	if (params.filePaths.length > 0) {
		const diffSpec: DiffSpec[] = [];
		for (const filePath of params.filePaths) {
			diffSpec.push({
				pathBytes: filePath,
				hunkHeaders: []
			});
		}

		const diffSpecJson = JSON.stringify(diffSpec);
		args.push('--diff-spec', diffSpecJson);
	}

	const stacks = listStacks({ project_directory: params.project_directory });

	if (stacks.length === 0) {
		throw new Error('No stacks found');
	}

	for (const stack of stacks) {
		if (stack.branchNames.includes(params.branch)) {
			if (stack.branchNames.length === 1) {
				// If this stack has only one branch, we can commit directly to it
				const branchRef = getBranchRef(params.branch);
				args.push('-s', branchRef);
				args.push('--parent', stack.tip);
				return executeGitButlerCommand(params.project_directory, args, undefined);
			}

			const stackBranches = listStackBranches({
				project_directory: params.project_directory,
				stack_id: stack.id
			});
			if (stackBranches.length === 0) {
				throw new Error(`No branches found in stack ${stack.id}`);
			}

			const branch = stackBranches.find((b) => b.name === params.branch);
			if (!branch) {
				throw new Error(`Branch ${params.branch} not found in stack ${stack.id}`);
			}

			if (branch.archived) {
				throw new Error(`Branch ${params.branch} is archived`);
			}

			const branchRef = getBranchRef(params.branch);
			args.push('-s', branchRef);
			args.push('--parent', branch.tip);
			return executeGitButlerCommand(params.project_directory, args, undefined);
		}
	}

	throw new Error(`Branch ${params.branch} not found in any stack`);
}

const TOOL_LISTINGS = [
	{
		name: 'commit',
		description: 'Commit a set of changes to a specific branch in the GitButler project.',
		inputSchema: zodToJsonSchema(CommitParamsSchema)
	}
] as const;

type ToolName = (typeof TOOL_LISTINGS)[number]['name'];

function isToolName(name: string): name is ToolName {
	return TOOL_LISTINGS.some((tool) => tool.name === name);
}

export function getCommitToolListing() {
	if (!hasGitButlerExecutable()) {
		return [];
	}

	return TOOL_LISTINGS;
}

export async function getCommitToolRequestHandler(
	toolName: string,
	params: Record<string, unknown>
): Promise<CallToolResult | null> {
	if (!isToolName(toolName) || !hasGitButlerExecutable()) {
		return null;
	}

	switch (toolName) {
		case 'commit': {
			try {
				const parsedParams = CommitParamsSchema.parse(params);
				commit(parsedParams);
				return { content: [{ type: 'text', text: 'Commit successful' }] };
			} catch (error: unknown) {
				if (error instanceof Error) {
					return { content: [{ type: 'text', text: `Error: ${error.message}` }], isError: true };
				}

				return { content: [{ type: 'text', text: `Error: ${String(error)}` }], isError: true };
			}
		}
	}
}
