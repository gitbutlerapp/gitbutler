import { LookupProjectResponseSchema, ProjectSchema } from '../shared/entities/index.js';
import {
	getGitbutlerAPIUrl,
	gitbutlerAPIRequest,
	hasGitButlerAPIKey,
	interpolatePath
} from '../shared/request.js';
import { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

enum ProjectAPIEnpoint {
	Projects = '/projects',
	Project = '/projects/{id}',
	RecentlyInteracted = '/projects/recently_interacted',
	RecentlyPushed = '/projects/recently_pushed',
	Lookup = '/projects/lookup/{owner}/{repo}',
	FullLookup = '/projects/full/{owner}/{repo}'
}

const ListProjectsParamsSchema = z.object({
	since: z.string({ description: 'Only list projects updated since this date' }).optional(),
	before: z.string({ description: 'Only list projects updated before this date' }).optional(),
	limit: z.number({ description: 'Limit the number of results listed' }).optional()
});

type ListProjectsParams = z.infer<typeof ListProjectsParamsSchema>;

/**
 * Return all projects
 */
async function listAllProjects(params: ListProjectsParams) {
	const url = getGitbutlerAPIUrl(ProjectAPIEnpoint.Projects, params);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ProjectSchema.array().parse(response);
	return parsed;
}

const GetProjectParamsSchema = z.object({
	id: z.string({ description: 'The ID of the project' })
});

type GetProjectParams = z.infer<typeof GetProjectParamsSchema>;

/**
 * Return a specific project
 */
async function getProject(params: GetProjectParams) {
	const apiPath = interpolatePath(ProjectAPIEnpoint.Project, params);
	const url = getGitbutlerAPIUrl(apiPath);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ProjectSchema.parse(response);
	return parsed;
}

/**
 * Return all projects that have been recently interacted with
 */
async function listRecentlyInteractedProjects() {
	const url = getGitbutlerAPIUrl(ProjectAPIEnpoint.RecentlyInteracted);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ProjectSchema.array().parse(response);
	return parsed;
}

/**
 * Return all projects that have been recently pushed to
 */
async function listRecentlyPushedProjects() {
	const url = getGitbutlerAPIUrl(ProjectAPIEnpoint.RecentlyPushed);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ProjectSchema.array().parse(response);
	return parsed;
}

const LookupProjectParamsSchema = z.object({
	owner: z.string({ description: 'The owner of the project' }),
	repo: z.string({ description: 'The slug of the project' })
});

type LookupProjectParams = z.infer<typeof LookupProjectParamsSchema>;

/**
 * Lookup a project by owner and repo name
 */
async function lookupProject(params: LookupProjectParams) {
	const apiPath = interpolatePath(ProjectAPIEnpoint.Lookup, params);
	const url = getGitbutlerAPIUrl(apiPath);
	const response = await gitbutlerAPIRequest(url);
	const parsed = LookupProjectResponseSchema.parse(response);
	return parsed;
}

/**
 * Lookup a project by owner and repo name, returning the full project object
 */
async function fullLookupProject(params: LookupProjectParams) {
	const apiPath = interpolatePath(ProjectAPIEnpoint.FullLookup, params);
	const url = getGitbutlerAPIUrl(apiPath);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ProjectSchema.parse(response);
	return parsed;
}

const TOOL_LISTINGS = [
	{
		name: 'list_projects',
		description: 'List all the GitButler projects that are available',
		inputSchema: zodToJsonSchema(ListProjectsParamsSchema)
	},
	{
		name: 'get_project',
		description: 'Get a specific GitButler project',
		inputSchema: zodToJsonSchema(GetProjectParamsSchema)
	},
	{
		name: 'list_recently_interacted_projects',
		description: 'List all the GitButler projects that have been recently interacted with',
		inputSchema: zodToJsonSchema(z.object({}))
	},
	{
		name: 'list_recently_pushed_projects',
		description: 'List all the GitButler projects that have been recently pushed to',
		inputSchema: zodToJsonSchema(z.object({}))
	},
	{
		name: 'lookup_project',
		description: 'Lookup a GitButler project by owner and repo, returning the project ID',
		inputSchema: zodToJsonSchema(LookupProjectParamsSchema)
	},
	{
		name: 'full_lookup_project',
		description: 'Lookup a GitButler project by owner and repo, returning the full project object',
		inputSchema: zodToJsonSchema(LookupProjectParamsSchema)
	}
] as const;

type ToolName = (typeof TOOL_LISTINGS)[number]['name'];

function isToolName(name: string): name is ToolName {
	return TOOL_LISTINGS.some((tool) => tool.name === name);
}

export function getProjectToolListings() {
	if (!hasGitButlerAPIKey()) {
		return [];
	}
	return TOOL_LISTINGS;
}

export async function getProjectToolRequestHandler(
	toolName: string,
	args: Record<string, unknown>
): Promise<CallToolResult | null> {
	if (!isToolName(toolName) || !hasGitButlerAPIKey()) {
		return null;
	}

	switch (toolName) {
		case 'list_projects': {
			const listProjectsParams = ListProjectsParamsSchema.parse(args);
			const result = await listAllProjects(listProjectsParams);
			return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
		}
		case 'get_project': {
			const getProjectParams = GetProjectParamsSchema.parse(args);
			const result = await getProject(getProjectParams);
			return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
		}
		case 'list_recently_interacted_projects': {
			const result = await listRecentlyInteractedProjects();
			return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
		}
		case 'list_recently_pushed_projects': {
			const result = await listRecentlyPushedProjects();
			return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
		}
		case 'lookup_project': {
			const lookupProjectParams = LookupProjectParamsSchema.parse(args);
			const result = await lookupProject(lookupProjectParams);
			return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
		}
		case 'full_lookup_project': {
			const lookupProjectParams = LookupProjectParamsSchema.parse(args);
			const result = await fullLookupProject(lookupProjectParams);
			return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
		}
	}
}
