import type { FileChange } from '$lib/ai/types';
type FileDiffAction = 'ADDED' | 'DELETED' | 'MODIFIED';

function formatAddedDiffs(diffs: string[]): string {
	const buffer: string[] = [];
	for (const diff of diffs) {
		const lines = diff.split('\n');
		for (const line of lines) {
			if (line.startsWith('@@')) continue;
			if (line.startsWith('+')) {
				const cleanLine = line.slice(1);
				buffer.push(cleanLine);
			}
		}
		buffer.push('\n');
	}
	return buffer.join('\n');
}

function formatDeletedDiffs(diffs: string[]): string {
	const buffer: string[] = [];
	for (const diff of diffs) {
		const lines = diff.split('\n');
		for (const line of lines) {
			if (line.startsWith('@@')) continue;
			if (line.startsWith('-')) {
				const cleanLine = line.slice(1);
				buffer.push(cleanLine);
			}
		}
		buffer.push('\n');
	}

	return buffer.join('\n');
}

function formatModifiedDiffs(diffs: string[], extension: string): string {
	const buffer: string[] = [];
	for (const diff of diffs) {
		const beforeBuffer: string[] = [];
		const afterBuffer: string[] = [];
		const lines = diff.split('\n');
		for (const line of lines) {
			if (line.startsWith('@@')) continue;
			if (line.startsWith('+')) {
				const cleanLine = line.slice(1);
				afterBuffer.push(cleanLine);
				continue;
			}

			if (line.startsWith('-')) {
				const cleanLine = line.slice(1);
				beforeBuffer.push(cleanLine);
				continue;
			}

			if (line.startsWith(' ')) {
				const cleanLine = line.slice(1);
				beforeBuffer.push(cleanLine);
				afterBuffer.push(cleanLine);
				continue;
			}
		}
		const before = beforeBuffer.join('\n');
		const after = afterBuffer.join('\n');
		buffer.push(
			`BEFORE:
\`\`\`${extension}
${before}
\`\`\`

AFTER:
\`\`\`${extension}
${after}
\`\`\`
`
		);
		buffer.push('\n-------------------------------------\n');
	}

	return buffer.join('\n');
}

function formatDiffs(diffs: string[], action: FileDiffAction, extension: string): string {
	switch (action) {
		case 'ADDED':
			return formatAddedDiffs(diffs);
		case 'DELETED':
			return formatDeletedDiffs(diffs);
		case 'MODIFIED':
			return formatModifiedDiffs(diffs, extension);
	}
}

function isAddedDiff(diff: string): boolean {
	const firstLine = diff.split('\n')[0];
	return firstLine?.startsWith('@@ -1,0') ?? false;
}

function isDeletedDiff(diff: string): boolean {
	const firstLine = diff.split('\n')[0];
	return firstLine?.endsWith('+1,0 @@') ?? false;
}

function determineActions(diffs: string[]): FileDiffAction {
	if (diffs.length === 1 && isAddedDiff(diffs[0]!)) {
		return 'ADDED';
	}

	if (diffs.length === 1 && isDeletedDiff(diffs[0]!)) {
		return 'DELETED';
	}

	{
		return 'MODIFIED';
	}
}

export function formatStagedChanges(stagedChanges: FileChange[]): string {
	return stagedChanges
		.map((change) => {
			const action = determineActions(change.diffs);
			const extension = change.path.split('.').pop() ?? '';
			return `
FILE PATH ${action}: ${change.path}

CHANGE CONTENT:
${formatDiffs(change.diffs, action, extension)}`;
		})
		.join('\n\n ================================= \n\n')
		.trim();
}
