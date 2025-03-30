#!/usr/bin/env node
import { VERSION } from './shared/version.js';
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { CallToolRequestSchema, ListToolsRequestSchema } from '@modelcontextprotocol/sdk/types.js';
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
			tools: {}
		}
	}
);

server.setRequestHandler(ListToolsRequestSchema, async () => {
	return {
		tools: []
	};
});

server.setRequestHandler(CallToolRequestSchema, async (request) => {
	try {
		if (!request.params.arguments) {
			throw new Error('No arguments provided');
		}
		return {
			content: []
		};
	} catch (error) {
		if (error instanceof z.ZodError) {
			throw new Error(`Validation error: ${JSON.stringify(error.errors)}`);
		}
		throw error;
	}
});
