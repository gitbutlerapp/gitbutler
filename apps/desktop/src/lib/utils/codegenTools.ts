import type { ToolCall } from '$lib/codegen/messages';
import type iconsJson from '@gitbutler/ui/data/icons.json';

export type IconName = keyof typeof iconsJson;

/**
 * Maps tool call names to appropriate icons based on partial string matching
 */
export function getToolIcon(toolName: string): IconName {
	const name = toolName.toLowerCase();

	// Partial matches for tool types
	if (name.includes('ls') || name.includes('list') || name.includes('directory')) {
		return 'folder';
	}
	if (name.includes('read') || name.includes('file')) {
		return 'docs-small';
	}
	if ((name.includes('write') && !name.includes('todo')) || name.includes('edit')) {
		return 'edit';
	}
	if (name.includes('todo')) {
		return 'checklist';
	}
	if (name.includes('grep') || name.includes('search')) {
		return 'search';
	}
	if (name.includes('bash') || name.includes('terminal') || name.includes('shell')) {
		return 'logs';
	}
	if (name.includes('batch') || name.includes('script')) {
		return 'text';
	}
	if (name.includes('exit')) {
		return 'signout';
	}

	// Default icon for unknown tool types
	return 'settings';
}

const KNOWN_TOOLS = [
	'Read',
	'Edit',
	'Write',
	'Bash',
	'Grep',
	'Glob',
	'Task',
	'TodoWrite',
	'WebFetch',
	'WebSearch'
] as const;

export function formatToolCall(toolCall: ToolCall): string {
	const input = toolCall.input;

	switch (toolCall.name) {
		case 'Read':
			return input['file_path'];

		case 'Edit':
		case 'Write':
			return input['file_path'];

		case 'Bash':
			// Show description if available, otherwise truncate command
			return truncate(input['command'], 128);

		case 'Grep':
			return `"${input['pattern']}"${input['path'] ? ` in ${input['path']}` : ''}`;

		case 'Glob':
			return `"${input['pattern']}"${input['path'] ? ` in ${input['path']}` : ''}`;

		case 'Task':
			return input['description'] || 'Running subtask';

		case 'TodoWrite': {
			const todos = input['todos'];
			return Array.isArray(todos)
				? `${todos.length} todo${todos.length !== 1 ? 's' : ''}`
				: 'todos';
		}

		case 'WebFetch':
			return truncate(input['url'], 50);

		case 'WebSearch':
			return `"${truncate(input['query'], 50)}"`;

		default: {
			// Log unknown tool types for debugging
			if (!KNOWN_TOOLS.includes(toolCall.name as any)) {
				console.warn('Unknown tool call type:', {
					name: toolCall.name,
					input: toolCall.input,
					result: toolCall.result
				});
			}

			// Fallback: try to find a meaningful value
			const keys = Object.keys(input);
			if (keys.length === 0) return 'no parameters';
			if (keys.length === 1) {
				const value = input[keys[0]!];
				return typeof value === 'string' ? truncate(value, 60) : JSON.stringify(value);
			}
			return truncate(JSON.stringify(input), 60);
		}
	}
}

function truncate(str: string, maxLength: number): string {
	if (str.length <= maxLength) return str;
	return str.slice(0, maxLength - 1) + '…';
}
