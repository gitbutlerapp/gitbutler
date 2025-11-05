type HunkLineSelector =
	| `#hunk-line-${string}\\:R${number} [data-testid="hunk-count-column"]`
	| `#hunk-line-${string}\\:L${number} [data-testid="hunk-count-column"]`;

function escapeFileName(fileName: string): string {
	return fileName.replace(/([.*+?^=!:${}()|[\]/\\])/g, '\\$1');
}

/**
 * Get the selector for a specific line in a hunk.
 */
export function getHunkLineSelector(
	fileName: string,
	lineNumber: number,
	type: 'left' | 'right'
): HunkLineSelector {
	const escapedFileName = escapeFileName(fileName);
	switch (type) {
		case 'left':
			return `#hunk-line-${escapedFileName}\\:L${lineNumber} [data-testid="hunk-count-column"]`;
		case 'right':
			return `#hunk-line-${escapedFileName}\\:R${lineNumber} [data-testid="hunk-count-column"]`;
	}
}
