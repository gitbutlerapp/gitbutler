import { LookupProjectResponseSchema, ProjectSchema } from '../shared/entities/index.js';
import { getGitbutlerAPIUrl, gitbutlerAPIRequest, interpolatePath } from '../shared/request.js';
import { z } from 'zod';

enum ProjectAPIEnpoint {
	Projects = '/projects',
	Project = '/projects/{id}',
	RecentlyInteracted = '/projects/recently_interacted',
	RecentlyPushed = '/projects/recently_pushed',
	Lookup = '/projects/lookup/{owner}/{repo}',
	FullLookup = '/projects/full/{owner}/{repo}'
}

export const ListProjectsParamsSchema = z.object({
	since: z.string({ description: 'Only list projects updated since this date' }).optional(),
	before: z.string({ description: 'Only list projects updated before this date' }).optional(),
	limit: z.number({ description: 'Limit the number of results listed' }).optional()
});

export type ListProjectsParams = z.infer<typeof ListProjectsParamsSchema>;

/**
 * Return all projects
 */
export async function listAllProjects(params: ListProjectsParams) {
	const url = getGitbutlerAPIUrl(ProjectAPIEnpoint.Projects, params);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ProjectSchema.array().parse(response);
	return parsed;
}

export const GetProjectParamsSchema = z.object({
	id: z.string({ description: 'The ID of the project' })
});

export type GetProjectParams = z.infer<typeof GetProjectParamsSchema>;

/**
 * Return a specific project
 */
export async function getProject(params: GetProjectParams) {
	const apiPath = interpolatePath(ProjectAPIEnpoint.Project, params);
	const url = getGitbutlerAPIUrl(apiPath);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ProjectSchema.parse(response);
	return parsed;
}

/**
 * Return all projects that have been recently interacted with
 */
export async function listRecentlyInteractedProjects() {
	const url = getGitbutlerAPIUrl(ProjectAPIEnpoint.RecentlyInteracted);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ProjectSchema.array().parse(response);
	return parsed;
}

/**
 * Return all projects that have been recently pushed to
 */
export async function listRecentlyPushedProjects() {
	const url = getGitbutlerAPIUrl(ProjectAPIEnpoint.RecentlyPushed);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ProjectSchema.array().parse(response);
	return parsed;
}

export const LookupProjectParamsSchema = z.object({
	owner: z.string({ description: 'The owner of the project' }),
	repo: z.string({ description: 'The slug of the project' })
});

export type LookupProjectParams = z.infer<typeof LookupProjectParamsSchema>;

/**
 * Lookup a project by owner and repo name
 */
export async function lookupProject(params: LookupProjectParams) {
	const apiPath = interpolatePath(ProjectAPIEnpoint.Lookup, params);
	const url = getGitbutlerAPIUrl(apiPath);
	const response = await gitbutlerAPIRequest(url);
	const parsed = LookupProjectResponseSchema.parse(response);
	return parsed;
}

/**
 * Lookup a project by owner and repo name, returning the full project object
 */
export async function fullLookupProject(params: LookupProjectParams) {
	const apiPath = interpolatePath(ProjectAPIEnpoint.FullLookup, params);
	const url = getGitbutlerAPIUrl(apiPath);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ProjectSchema.parse(response);
	return parsed;
}
