#!/usr/bin/env node
import { VERSION } from './shared/version.js';
import * as chatMessages from './tools/chatMessages.js';
import * as branch from './tools/client/branch.js';
import * as commit from './tools/client/commit.js';
import * as status from './tools/client/status.js';
import * as patchStacks from './tools/patchStacks.js';
import * as projects from './tools/projects.js';
import * as rules from './tools/rules.js';
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
	CallToolRequestSchema,
	GetPromptRequestSchema,
	ListPromptsRequestSchema,
	ListToolsRequestSchema
} from '@modelcontextprotocol/sdk/types.js';
import fetch from 'node-fetch';
import { z } from 'zod';

if (globalThis.fetch) {
	globalThis.fetch = fetch as unknown as typeof global.fetch;
}

const server = new Server(
	{
		name: 'gitbutler-mcp-server',
		version: VERSION
	},
	{
		instructions: `This is the GitButler MCP server.
It provides tools for managing Git repositories, including commit, branch, status, and patch stack management.
You can use the tools to interact with your Git repositories.`,
		capabilities: {
			tools: { listChanged: true },
			prompts: { listChanged: true }
		}
	}
);

server.setRequestHandler(ListToolsRequestSchema, async () => {
	return {
		tools: [
			// ...projects.getProjectToolListings(),
			// ...chatMessages.getChatMessageToolListings(),
			// ...patchStacks.getPatchStackToolListing(),
			...rules.getRulesToolListings(),
			...status.getStatusToolListing(),
			...commit.getCommitToolListing(),
			...branch.getBranchToolListing()
		],
		prompts: [...commit.getCommitToolPrompts()]
	};
});

server.setRequestHandler(ListPromptsRequestSchema, async () => {
	return {
		prompts: [...commit.getCommitToolPrompts()]
	};
});

server.setRequestHandler(CallToolRequestSchema, async (request) => {
	try {
		if (!request.params.arguments) {
			throw new Error('No arguments provided');
		}

		const handlers = [
			rules.getRulesToolRequestHandler,
			projects.getProjectToolRequestHandler,
			chatMessages.getChatMesssageToolRequestHandler,
			patchStacks.getPatchStackToolRequestHandler,
			status.getStatusToolRequestHandler,
			commit.getCommitToolRequestHandler,
			branch.getBranchToolRequestHandler
		];

		for (const handler of handlers) {
			const result = await handler(request.params.name, request.params.arguments);
			if (result === null) continue;
			return result;
		}

		throw new Error(`Unknown tool: ${request.params.name}`);
	} catch (error) {
		if (error instanceof z.ZodError) {
			return {
				isError: true,
				content: [
					{
						type: 'text',
						text: `Invalid parameters for tool ${request.params.name}: ${error.message}`
					}
				]
			};
		}
		return { isError: true, content: [{ type: 'text', text: `Error: ${String(error)}` }] };
	}
});

server.setRequestHandler(GetPromptRequestSchema, async (request) => {
	if (!request.params.name) {
		return {
			isError: true,
			content: [{ type: 'text', text: 'No prompt name provided' }]
		};
	}

	const handlers = [commit.getCommitToolPromptRequestHandler];
	for (const handler of handlers) {
		const result = await handler(request.params.name, request.params.arguments ?? {});
		if (result === null) continue;
		return result;
	}

	return {
		isError: true,
		content: [{ type: 'text', text: `Unknown prompt: ${request.params.name}` }]
	};
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
