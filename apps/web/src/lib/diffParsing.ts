export function splitDiffIntoHunks(diff: string): string[] {
	const hunkRegex = /(@@[^@]*@@)/g;
	const matches = diff.split(hunkRegex);
	const hunks: string[] = [];

	for (let i = 0; i < matches.length; i++) {
		if (matches[i].startsWith('@@')) {
			const hunkHeader = matches[i];
			const hunkBody = matches[i + 1] || '';
			hunks.push(hunkHeader + hunkBody);
		}
	}

	return hunks;
}
