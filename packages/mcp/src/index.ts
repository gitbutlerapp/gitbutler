#!/usr/bin/env node
import { VERSION } from './shared/version.js';
import * as chatMessages from './tools/chatMessages.js';
import * as patchStacks from './tools/patchStacks.js';
import * as projects from './tools/projects.js';
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { CallToolRequestSchema, ListToolsRequestSchema } from '@modelcontextprotocol/sdk/types.js';
import fetch from 'node-fetch';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

if (globalThis.fetch) {
	globalThis.fetch = fetch as unknown as typeof global.fetch;
}

const server = new Server(
	{
		name: 'gitbutler-mcp-server',
		version: VERSION
	},
	{
		capabilities: {
			tools: {}
		}
	}
);

server.setRequestHandler(ListToolsRequestSchema, async () => {
	return {
		tools: [
			{
				name: 'list_projects',
				description: 'List all the GitButler projects that are available',
				inputSchema: zodToJsonSchema(projects.ListProjectsParamsSchema)
			},
			{
				name: 'get_project',
				description: 'Get a specific GitButler project',
				inputSchema: zodToJsonSchema(projects.GetProjectParamsSchema)
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
				inputSchema: zodToJsonSchema(projects.LookupProjectParamsSchema)
			},
			{
				name: 'full_lookup_project',
				description:
					'Lookup a GitButler project by owner and repo, returning the full project object',
				inputSchema: zodToJsonSchema(projects.LookupProjectParamsSchema)
			},
			{
				name: 'get_chat_messages_for_patch',
				description: 'Get all review chat messages for a given patch',
				inputSchema: zodToJsonSchema(chatMessages.GetChatMessagesForPatchParamsSchema)
			},
			{
				name: 'list_patch_stacks',
				description: 'List all the patch stacks for a given project',
				inputSchema: zodToJsonSchema(patchStacks.GetProjectPatchStacksParamsSchema)
			},
			{
				name: 'get_patch_stack',
				description: 'Get a specific patch stack by UUID',
				inputSchema: zodToJsonSchema(patchStacks.GetPatchStackParamsSchema)
			},
			{
				name: 'get_patch_commit',
				description:
					'Get a specific patch commit by branch UUID and change ID. This includes information about the file changes',
				inputSchema: zodToJsonSchema(patchStacks.GetPatchCommitParamsSchema)
			}
		]
	};
});

server.setRequestHandler(CallToolRequestSchema, async (request) => {
	try {
		if (!request.params.arguments) {
			throw new Error('No arguments provided');
		}

		switch (request.params.name) {
			case 'list_projects': {
				const listProjectsParams = projects.ListProjectsParamsSchema.parse(
					request.params.arguments
				);
				const result = await projects.listAllProjects(listProjectsParams);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'get_project': {
				const getProjectParams = projects.GetProjectParamsSchema.parse(request.params.arguments);
				const result = await projects.getProject(getProjectParams);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'list_recently_interacted_projects': {
				const result = await projects.listRecentlyInteractedProjects();
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'list_recently_pushed_projects': {
				const result = await projects.listRecentlyPushedProjects();
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'lookup_project': {
				const lookupProjectParams = projects.LookupProjectParamsSchema.parse(
					request.params.arguments
				);
				const result = await projects.lookupProject(lookupProjectParams);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'full_lookup_project': {
				const lookupProjectParams = projects.LookupProjectParamsSchema.parse(
					request.params.arguments
				);
				const result = await projects.lookupProject(lookupProjectParams);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'get_chat_messages_for_patch': {
				const getChatMessagesParams = chatMessages.GetChatMessagesForPatchParamsSchema.parse(
					request.params.arguments
				);
				const result = await chatMessages.getChatMessagesForPatch(getChatMessagesParams);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'list_patch_stacks': {
				const listPatchStacksParams = patchStacks.GetProjectPatchStacksParamsSchema.parse(
					request.params.arguments
				);
				const result = await patchStacks.listAllPatchStacks(listPatchStacksParams);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'get_patch_stack': {
				const getPatchStackParams = patchStacks.GetPatchStackParamsSchema.parse(
					request.params.arguments
				);
				const result = await patchStacks.getPatchStack(getPatchStackParams);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			case 'get_patch_commit': {
				const getPatchCommitParams = patchStacks.GetPatchCommitParamsSchema.parse(
					request.params.arguments
				);
				const result = await patchStacks.getPatchCommit(getPatchCommitParams);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			}
			default:
				throw new Error(`Unknown tool: ${request.params.name}`);
		}
	} catch (error) {
		if (error instanceof z.ZodError) {
			throw new Error(`Validation error: ${JSON.stringify(error.errors)}`);
		}
		throw error;
	}
});

async function run() {
	const transport = new StdioServerTransport();
	await server.connect(transport);
	console.warn('GitButler MCP Server is running on stdio');
}

run().catch((error) => {
	console.error('Error starting server:', error);
	process.exit(1);
});
