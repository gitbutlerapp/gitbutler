export enum SectionType {
	AddedLines,
	RemovedLines,
	Context
}

export enum CountColumnSide {
	Before,
	After
}

export type Line = {
	readonly beforeLineNumber?: number;
	readonly afterLineNumber?: number;
	readonly content: string;
};

export type ContentSection = {
	readonly lines: Line[];
	readonly sectionType: SectionType;
};

export type Hunk = {
	readonly oldStart: number;
	readonly newStart: number;
	readonly contentSections: ContentSection[];
};

const headerRegex =
	/@@ -(?<beforeStart>\d+),?(?<beforeCount>\d+)? \+(?<afterStart>\d+),?(?<afterCount>\d+) @@(?<comment>.+)?/;
function parseHeader(header: string): { oldStart: number; newStart: number } {
	const result = headerRegex.exec(header);
	if (!result?.groups) {
		throw new Error('Failed to parse diff header');
	}
	return {
		oldStart: parseInt(result.groups['beforeStart']),
		newStart: parseInt(result.groups['afterStart'])
	};
}

function lineType(line: string): SectionType {
	if (line.startsWith('+')) {
		return SectionType.AddedLines;
	} else if (line.startsWith('-')) {
		return SectionType.RemovedLines;
	} else {
		return SectionType.Context;
	}
}

export function parsePatch(patch: string) {
	const lines = patch.trim().split('\n');
	console.log(lines);

	const hunks = [];
	let currentHunk: Hunk | undefined;
	// These zero values will never get used in practice.
	let lastBefore = 0;
	let lastAfter = 0;

	for (const line of lines) {
		if (line.startsWith('@@')) {
			currentHunk = {
				...parseHeader(line),
				contentSections: []
			};
			hunks.push(currentHunk);
			lastBefore = currentHunk.oldStart;
			lastAfter = currentHunk.newStart;
			continue;
		}
		if (!currentHunk) {
			continue;
		}

		const type = lineType(line);
		let lastSection = currentHunk.contentSections.at(-1);
		if (!lastSection) {
			lastSection = { lines: [], sectionType: type };
			currentHunk.contentSections.push(lastSection);
		}

		// If the type has changed, we want to start a new section
		if (lastSection.sectionType !== type) {
			lastSection = { lines: [], sectionType: type };
			currentHunk.contentSections.push(lastSection);
		}

		if (type === SectionType.AddedLines) {
			lastAfter += 1;
			lastSection.lines.push({ afterLineNumber: lastAfter, content: line.slice(1) });
		} else if (type === SectionType.RemovedLines) {
			lastBefore += 1;
			lastSection.lines.push({ beforeLineNumber: lastBefore, content: line.slice(1) });
		} else {
			lastAfter += 1;
			lastBefore += 1;
			lastSection.lines.push({
				afterLineNumber: lastAfter,
				beforeLineNumber: lastBefore,
				content: line.slice(1)
			});
		}
	}

	console.log(hunks);
	return hunks;
}
