export function splitDiffIntoHunks(diff: string): string[] {
	const lines = diff.split('\n');

	const hunks: string[] = [];

	// The only time we should see a line start with @@ in a diff or patch, is
	// when there ia a hunk header.
	for (const line of lines) {
		if (line.startsWith('@@')) {
			hunks.push('');
		}

		const lastIndex = hunks.length - 1;
		if (lastIndex >= 0) {
			hunks[lastIndex] = `${hunks[lastIndex]}${line}\n`;
		}
	}

	// Remove trailing newlines
	for (const index in hunks) {
		if (hunks[index].endsWith('\n')) {
			hunks[index] = hunks[index].slice(0, -1);
		}
	}

	return hunks;
}
