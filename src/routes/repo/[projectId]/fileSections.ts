import type { File, Hunk } from '$lib/vbranches';

export type Line = {
	beforeLineNumber: number | undefined;
	afterLineNumber: number | undefined;
	content: string;
};

export type HunkHeader = {
	beforeStart: number;
	beforeLength: number;
	afterStart: number;
	afterLength: number;
};

export type HunkSection = {
	hunk: Hunk;
	header: HunkHeader;
	subSections: Section[];
};

export enum SectionType {
	AddedLines,
	RemovedLines,
	Context
}

export type Section = {
	linesShown: number;
	lines: Line[];
	sectionType: SectionType;
};

export function parseHunkHeader(header: string | undefined): HunkHeader {
	if (!header) {
		return { beforeStart: 0, beforeLength: 0, afterStart: 0, afterLength: 0 };
	}
	const [before, after] = header.split('@@')[1].trim().split(' ');
	const [beforeStart, beforeLength] = before.split(',').map((n) => Math.abs(parseInt(n, 10)));
	const [afterStart, afterLength] = after.split(',').map((n) => Math.abs(parseInt(n, 10)));
	return { beforeStart, beforeLength, afterStart, afterLength };
}

export function parseHunkSection(hunk: Hunk): HunkSection {
	const lines = hunk.diff.split('\n');
	const header = parseHunkHeader(lines.shift());
	const hunkSection: HunkSection = {
		hunk: hunk,
		header: header,
		subSections: []
	};

	let currentBeforeLineNumber = header.beforeStart;
	let currentAfterLineNumber = header.afterStart;

	let currentSection: Section | undefined;
	while (lines.length > 0) {
		const line = lines.shift();
		if (!line) break;
		if (line.startsWith('-')) {
			if (!currentSection || currentSection.sectionType != SectionType.RemovedLines) {
				if (currentSection) hunkSection.subSections.push(currentSection);
				currentSection = { linesShown: 1, lines: [], sectionType: SectionType.RemovedLines };
			}
			currentSection.lines.push({
				beforeLineNumber: currentBeforeLineNumber,
				afterLineNumber: undefined,
				content: line.slice(1)
			});
			currentBeforeLineNumber++;
		} else if (line.startsWith('+')) {
			if (!currentSection || currentSection.sectionType != SectionType.AddedLines) {
				if (currentSection) hunkSection.subSections.push(currentSection);
				currentSection = { linesShown: 1, lines: [], sectionType: SectionType.AddedLines };
			}
			currentSection.lines.push({
				beforeLineNumber: undefined,
				afterLineNumber: currentAfterLineNumber,
				content: line.slice(1)
			});
			currentAfterLineNumber++;
		} else {
			if (!currentSection || currentSection.sectionType != SectionType.Context) {
				if (currentSection) hunkSection.subSections.push(currentSection);
				currentSection = { linesShown: 0, lines: [], sectionType: SectionType.Context };
			}
			currentSection.lines.push({
				beforeLineNumber: currentBeforeLineNumber,
				afterLineNumber: currentAfterLineNumber,
				content: line.slice(1)
			});
			currentBeforeLineNumber++;
			currentAfterLineNumber++;
		}
	}
	if (currentSection && currentSection.lines.length > 0) {
		hunkSection.subSections.push(currentSection);
	}
	return hunkSection;
}

export function parseFileSections(file: File): (Section | HunkSection)[] {
	const hunkSections = file.hunks
		.map(parseHunkSection)
		.filter((hunkSection) => hunkSection !== undefined);

	const lines = file.content.split('\n');
	const sections: (Section | HunkSection)[] = [];

	let currentBeforeLineNumber = 1;
	let currentAfterLineNumber = 1;
	let currentContext: Section | undefined = undefined;

	let i = 0;
	while (i < lines.length) {
		if (currentContext === undefined) {
			currentContext = { linesShown: 0, lines: [], sectionType: SectionType.Context };
		}
		const nextHunk = hunkSections.at(0);
		if (
			!nextHunk ||
			(currentBeforeLineNumber < nextHunk.header.beforeStart &&
				currentAfterLineNumber < nextHunk.header.afterStart)
		) {
			// add line to current context
			currentContext.lines.push({
				beforeLineNumber: currentBeforeLineNumber,
				afterLineNumber: currentAfterLineNumber,
				content: lines[i]
			});
			currentBeforeLineNumber++;
			currentAfterLineNumber++;
			i++;
			continue;
		} else {
			// flush current context
			if (currentContext.lines.length > 0) {
				sections.push(currentContext);
				currentContext = undefined;
			}
			// add next hunk section and skip over to context after hunk
			hunkSections.shift();
			sections.push(nextHunk);
			currentBeforeLineNumber = nextHunk.header.beforeStart + nextHunk.header.beforeLength;
			currentAfterLineNumber = nextHunk.header.afterStart + nextHunk.header.afterLength;
			// Jump over to the context after the hunk
			i =
				nextHunk.header.beforeLength > 0
					? (i = currentBeforeLineNumber - 1)
					: (i = currentAfterLineNumber - 1);
			continue;
		}
	}
	if (currentContext !== undefined && currentContext.lines.length > 0) {
		sections.push(currentContext);
	}

	return sections;
}
