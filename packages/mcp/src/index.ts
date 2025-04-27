#!/usr/bin/env node
import { VERSION } from './shared/version.js';
import * as chatMessages from './tools/chatMessages.js';
import * as commit from './tools/client/commit.js';
import * as status from './tools/client/status.js';
import * as patchStacks from './tools/patchStacks.js';
import * as projects from './tools/projects.js';
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
	CallToolRequestSchema,
	GetPromptRequestSchema,
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
		capabilities: {
			tools: {},
			prompts: {}
		}
	}
);

server.setRequestHandler(ListToolsRequestSchema, async () => {
	return {
		tools: [
			...projects.getProjectToolListings(),
			...chatMessages.getChatMessageToolListings(),
			...patchStacks.getPatchStackToolListing(),
			...status.getStatusToolListing(),
			...commit.getCommitToolListing()
		],
		prompts: [...commit.getCommitToolPrompts()]
	};
});

server.setRequestHandler(CallToolRequestSchema, async (request) => {
	try {
		if (!request.params.arguments) {
			throw new Error('No arguments provided');
		}

		const handlers = [
			projects.getProjectToolRequestHandler,
			chatMessages.getChatMesssageToolRequestHandler,
			patchStacks.getPatchStackToolRequestHandler,
			status.getStatusToolRequestHandler,
			commit.getCommitToolRequestHandler
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
