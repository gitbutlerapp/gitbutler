import { BaseParamsSchema } from './shared.js';
import { executeGitButlerCommand, hasGitButlerExecutable } from '../../shared/command.js';
import { StackSchema } from '../../shared/entities/stacks.js';
import { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

const CreateBranchParamsSchema = BaseParamsSchema.extend({
	branch_name: z.string({ description: 'The name of the branch to create.' }),
	description: z
		.string({
			description:
				'The description of the branch should contain information about its purpose, todos, etc.'
		})
		.optional()
});

type CreateBranchParams = z.infer<typeof CreateBranchParamsSchema>;

/**
 * Create a new branch in the current GitButler project.
 */
export function createBranch(params: CreateBranchParams) {
	const args = ['stack-branches', '-b', params.branch_name];

	if (params.description) {
		args.push('-d', params.description);
	}

	return executeGitButlerCommand(params.project_directory, args, StackSchema);
}

const AddBranchToStackParamsSchema = BaseParamsSchema.extend({
	stack_id: z.string({ description: 'The ID of the stack to add the branch to.' }),
	branch_name: z.string({ description: 'The name of the branch to add.' })
});

type AddBranchToStackParams = z.infer<typeof AddBranchToStackParamsSchema>;

/**
 * Add a branch to a stack in the current GitButler project.
 */
export function addBranchToStack(params: AddBranchToStackParams) {
	const args = ['stack-branches', params.stack_id, '-b', params.branch_name];
	return executeGitButlerCommand(params.project_directory, args, StackSchema);
}

const TOOL_LISTINGS = [
	{
		name: 'create-branch',
		description: 'Create a new branch in the current GitButler project.',
		inputSchema: zodToJsonSchema(CreateBranchParamsSchema)
	},
	{
		name: 'add-branch-to-stack',
		description: 'Add a branch to an existing stack in the current GitButler project.',
		inputSchema: zodToJsonSchema(AddBranchToStackParamsSchema)
	}
] as const;

type ToolName = (typeof TOOL_LISTINGS)[number]['name'];

function isToolName(name: string): name is ToolName {
	return TOOL_LISTINGS.some((tool) => tool.name === name);
}

export function getBranchToolListing() {
	if (!hasGitButlerExecutable()) {
		return [];
	}
	return TOOL_LISTINGS;
}

export async function getBranchToolRequestHandler(
	toolName: string,
	params: Record<string, unknown>
): Promise<CallToolResult | null> {
	if (!isToolName(toolName) || !hasGitButlerExecutable()) {
		return null;
	}

	try {
		switch (toolName) {
			case 'create-branch': {
				const parsedParams = CreateBranchParamsSchema.parse(params);
				const result = createBranch(parsedParams);
				const branchName = result.heads[0]?.name;
				if (!branchName) {
					throw new Error('Branch name not found in the result');
				}

				return {
					content: [
						{
							type: 'text',
							text: `Stack containing branch ${branchName} created with id: ${result.id}`
						}
					]
				};
			}
			case 'add-branch-to-stack': {
				const parsedParams = AddBranchToStackParamsSchema.parse(params);
				const result = addBranchToStack(parsedParams);
				const branchName = result.heads[0]?.name;

				if (!branchName) {
					throw new Error('Branch name not found in the result');
				}

				return {
					content: [
						{
							type: 'text',
							text: `Branch ${branchName} added to stack with id: ${result.id}`
						}
					]
				};
			}
		}
	} catch (error: unknown) {
		if (error instanceof Error) {
			return { content: [{ type: 'text', text: error.message }], isError: true };
		}
		return { content: [{ type: 'text', text: String(error) }], isError: true };
	}
}
