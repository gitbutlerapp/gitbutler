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
	if (name.includes('glob') || name.includes('pattern')) {
		return 'filter-applied-small';
	}

	// Default icon for unknown tool types
	return 'settings';
}
