import { executeGitButlerCommand, hasGitButlerExecutable } from '../../shared/command.js';
import { UnifiedWorktreeChanges } from '../../shared/entities/changes.js';
import { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

const StatusParamsSchema = z.object({
	project_directory: z.string({ description: 'The absolute path to the project directory' })
});

type StatusParams = z.infer<typeof StatusParamsSchema>;

function status(params: StatusParams) {
	const args = ['status', '--unified-diff'];

	return executeGitButlerCommand(params.project_directory, args, UnifiedWorktreeChanges);
}

const TOOL_LISTINGS = [
	{
		name: 'get_unified_wortree_changes',
		description:
			'Get the file changes of the current GitButler project. Always call this tool when you want to get the file changes.',
		inputSchema: zodToJsonSchema(StatusParamsSchema)
	}
] as const;

type ToolName = (typeof TOOL_LISTINGS)[number]['name'];

function isToolName(name: string): name is ToolName {
	return TOOL_LISTINGS.some((tool) => tool.name === name);
}

export function getStatusToolListing() {
	if (!hasGitButlerExecutable()) {
		return [];
	}
	return TOOL_LISTINGS;
}

export async function getStatusToolRequestHandler(
	toolName: string,
	args: Record<string, unknown>
): Promise<CallToolResult | null> {
	if (!isToolName(toolName) || !hasGitButlerExecutable()) {
		return null;
	}

	switch (toolName) {
		case 'get_unified_wortree_changes': {
			try {
				const params = StatusParamsSchema.parse(args);
				const result = status(params);
				return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
			} catch (error: unknown) {
				if (error instanceof Error)
					return { content: [{ type: 'text', text: `Error: ${error.message}` }], isError: true };

				return { content: [{ type: 'text', text: `Error: ${String(error)}` }], isError: true };
			}
		}
	}
}
