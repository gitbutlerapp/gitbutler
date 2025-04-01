import { PatchWithFilesSchema } from '../shared/entities/patch.js';
import { PatchStackSchema } from '../shared/entities/patchStack.js';
import {
	getGitbutlerAPIUrl,
	gitbutlerAPIRequest,
	hasGitButlerAPIKey,
	interpolatePath
} from '../shared/request.js';
import { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

enum PatchStackAPIEndpoint {
	PatchStacks = '/patch_stack/{owner}/{project}/',
	PatchStack = '/patch_stack/{uuid}',
	PatchCommit = '/patch_stack/{branchUuid}/patch/{changeId}'
}

const GetProjectPatchStacksParamsSchema = z.object({
	owner: z.string({ description: 'The owner of the project' }),
	project: z.string({ description: 'The slug of the project' }),
	branch_id: z.string({ description: 'Filter by branch ID' }).optional(),
	status: z
		.enum(['active', 'inactive', 'closed', 'loading', 'all'], {
			description: 'Filter by stack status'
		})
		.optional(),
	limit: z.number({ description: 'Limit the number of results listed' }).optional()
});

type ListProjectsParams = z.infer<typeof GetProjectPatchStacksParamsSchema>;

/**
 * Return all the patch stacks for a project
 */
async function listAllPatchStacks(params: ListProjectsParams) {
	const interpolationParams = {
		owner: params.owner,
		project: params.project
	};

	const queryParams = {
		branch_id: params.branch_id,
		status: params.status,
		limit: params.limit
	};

	const apiPath = interpolatePath(PatchStackAPIEndpoint.PatchStacks, interpolationParams);
	const url = getGitbutlerAPIUrl(apiPath, queryParams);
	const response = await gitbutlerAPIRequest(url);
	const parsed = PatchStackSchema.array().parse(response);
	return parsed;
}

const GetPatchStackParamsSchema = z.object({
	uuid: z.string({ description: 'The UUID of the patch stack' })
});

type GetPatchStackParams = z.infer<typeof GetPatchStackParamsSchema>;

/**
 * Return a patch stack
 */
async function getPatchStack(params: GetPatchStackParams) {
	const interpolationParams = {
		uuid: params.uuid
	};

	const apiPath = interpolatePath(PatchStackAPIEndpoint.PatchStack, interpolationParams);
	const url = getGitbutlerAPIUrl(apiPath);
	const response = await gitbutlerAPIRequest(url);
	const parsed = PatchStackSchema.parse(response);
	return parsed;
}

const GetPatchCommitParamsSchema = z.object({
	branchUuid: z.string({ description: 'The UUID of the branch' }),
	changeId: z.string({ description: 'The ID of the change' })
});

type GetPatchCommitParams = z.infer<typeof GetPatchCommitParamsSchema>;

/**
 * Return a patch commit
 */
async function getPatchCommit(params: GetPatchCommitParams) {
	const interpolationParams = {
		branchUuid: params.branchUuid,
		changeId: params.changeId
	};

	const apiPath = interpolatePath(PatchStackAPIEndpoint.PatchCommit, interpolationParams);
	const url = getGitbutlerAPIUrl(apiPath);
	const response = await gitbutlerAPIRequest(url);
	const parsed = PatchWithFilesSchema.parse(response);
	return parsed;
}

const TOOL_LISTINGS = [
	{
		name: 'list_patch_stacks',
		description: 'List all the patch stacks for a given project',
		inputSchema: zodToJsonSchema(GetProjectPatchStacksParamsSchema)
	},
	{
		name: 'get_patch_stack',
		description: 'Get a specific patch stack by UUID',
		inputSchema: zodToJsonSchema(GetPatchStackParamsSchema)
	},
	{
		name: 'get_patch_commit',
		description:
			'Get a specific patch commit by branch UUID and change ID. This includes information about the file changes',
		inputSchema: zodToJsonSchema(GetPatchCommitParamsSchema)
	}
] as const;

type ToolName = (typeof TOOL_LISTINGS)[number]['name'];

function isToolName(name: string): name is ToolName {
	return TOOL_LISTINGS.some((tool) => tool.name === name);
}

export function getPatchStackToolListing() {
	if (!hasGitButlerAPIKey()) return [];
	return TOOL_LISTINGS;
}

export async function getPatchStackToolRequestHandler(
	toolName: string,
	args: Record<string, unknown>
): Promise<CallToolResult | null> {
	if (!isToolName(toolName) || !hasGitButlerAPIKey()) {
		return null;
	}

	switch (toolName) {
		case 'list_patch_stacks': {
			const listPatchStacksParams = GetProjectPatchStacksParamsSchema.parse(args);
			const result = await listAllPatchStacks(listPatchStacksParams);
			return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
		}
		case 'get_patch_stack': {
			const getPatchStackParams = GetPatchStackParamsSchema.parse(args);
			const result = await getPatchStack(getPatchStackParams);
			return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
		}
		case 'get_patch_commit': {
			const getPatchCommitParams = GetPatchCommitParamsSchema.parse(args);
			const result = await getPatchCommit(getPatchCommitParams);
			return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
		}
	}
}
