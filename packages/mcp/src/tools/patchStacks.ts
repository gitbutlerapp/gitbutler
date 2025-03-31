import { PatchStackSchema } from '../shared/entities/patchStack.js';
import { getGitbutlerAPIUrl, gitbutlerAPIRequest, interpolatePath } from '../shared/request.js';
import { z } from 'zod';

enum PatchStackAPIEndpoint {
	PatchStacks = '/patch_stack/{owner}/{project}/',
	PatchStack = '/patch_stack/{uuid}'
}

export const GetProjectPatchStacksParamsSchema = z.object({
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

export type ListProjectsParams = z.infer<typeof GetProjectPatchStacksParamsSchema>;

/**
 * Return all the patch stacks for a project
 */
export async function listAllPatchStacks(params: ListProjectsParams) {
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

export const GetPatchStackParamsSchema = z.object({
	uuid: z.string({ description: 'The UUID of the patch stack' })
});

export type GetPatchStackParams = z.infer<typeof GetPatchStackParamsSchema>;

/**
 * Return a patch stack
 */
export async function getPatchStack(params: GetPatchStackParams) {
	const interpolationParams = {
		uuid: params.uuid
	};

	const apiPath = interpolatePath(PatchStackAPIEndpoint.PatchStack, interpolationParams);
	const url = getGitbutlerAPIUrl(apiPath);
	const response = await gitbutlerAPIRequest(url);
	const parsed = PatchStackSchema.parse(response);
	return parsed;
}
