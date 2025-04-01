import { ChatMessageSchema } from '../shared/entities/chatMessage.js';
import {
	getGitbutlerAPIUrl,
	gitbutlerAPIRequest,
	hasGitButlerAPIKey,
	interpolatePath
} from '../shared/request.js';
import { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

enum ChatMessageAPIEndpoint {
	ChatMessages = '/chat_messages/{projectId}/chats/{changeId}'
}

const GetChatMessagesForPatchParamsSchema = z.object({
	projectId: z.string({ description: 'The ID of the project' }),
	changeId: z.string({ description: 'The ID of the change' }),
	limit: z.number({ description: 'Limit the number of results listed' }).optional(),
	before: z.string({ description: 'Get messages before this date' }).optional(),
	since: z.string({ description: 'Get messages after this date' }).optional()
});

type GetChatMessagesForPatchParams = z.infer<typeof GetChatMessagesForPatchParamsSchema>;

/**
 * Return all chat messages for a patch
 */
async function getChatMessagesForPatch(params: GetChatMessagesForPatchParams) {
	const interpolationParams = {
		projectId: params.projectId,
		changeId: params.changeId
	};

	const queryParams = {
		limit: params.limit,
		before: params.before,
		since: params.since
	};

	const apiPath = interpolatePath(ChatMessageAPIEndpoint.ChatMessages, interpolationParams);
	const url = getGitbutlerAPIUrl(apiPath, queryParams);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ChatMessageSchema.array().parse(response);
	return parsed;
}

const TOOL_LISTINGS = [
	{
		name: 'get_chat_messages_for_patch',
		description: 'Get all review chat messages for a given patch',
		inputSchema: zodToJsonSchema(GetChatMessagesForPatchParamsSchema)
	}
] as const;

type ToolName = (typeof TOOL_LISTINGS)[number]['name'];

function isToolName(name: string): name is ToolName {
	return TOOL_LISTINGS.some((tool) => tool.name === name);
}

export function getChatMessageToolListings() {
	if (!hasGitButlerAPIKey()) return [];
	return TOOL_LISTINGS;
}

export async function getChatMesssageToolRequestHandler(
	toolName: string,
	args: Record<string, unknown>
): Promise<CallToolResult | null> {
	if (!isToolName(toolName) || !hasGitButlerAPIKey) {
		return null;
	}

	switch (toolName) {
		case 'get_chat_messages_for_patch': {
			const getChatMessagesParams = GetChatMessagesForPatchParamsSchema.parse(args);
			const result = await getChatMessagesForPatch(getChatMessagesParams);
			return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
		}
	}
}
