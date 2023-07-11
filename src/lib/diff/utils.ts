/**
 * Helper function to extract line numbers from the chunk header in a diff.
 *
 * @param diff A diff containing a chunk header
 * @returns The original and modified file line numbers
 */
export function getLinesFromChunkHeader(diff: string): {
	originalLineNumber: number;
	currentLineNumber: number;
} {
	try {
		const diffLines = diff.split('\n');
		const header = diffLines[0];
		const lr = header.split('@@')[1].trim().split(' ');
		return {
			originalLineNumber: parseInt(lr[0].split(',')[0].slice(1)),
			currentLineNumber: parseInt(lr[1].split(',')[0].slice(1))
		};
	} catch {
		return {
			originalLineNumber: -1,
			currentLineNumber: -1
		};
	}
}
