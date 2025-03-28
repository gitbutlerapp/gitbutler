import { LocalFile } from '$lib/files/file';
import { plainToInstance } from 'class-transformer';
import type { AnyFile } from '$lib/files/file';
import type { RemoteHunk } from '$lib/hunks/hunk';
import type { Hunk } from '$lib/hunks/hunk';

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

export enum SectionType {
	AddedLines,
	RemovedLines,
	Context
}

export enum CountColumnSide {
	Before,
	After
}

export class HunkSection {
	hunk!: Hunk;
	header!: HunkHeader;
	subSections!: ContentSection[];
	hasConflictMarkers!: boolean;

	get maxLineNumber(): number {
		const lastSection = this.subSections[this.subSections.length - 1];
		if (!lastSection) {
			return 0;
		}
		return Math.max(
			lastSection.lines[lastSection.lines.length - 1]?.afterLineNumber ?? 0,
			lastSection.lines[lastSection.lines.length - 1]?.beforeLineNumber ?? 0
		);
	}
}

export class ContentSection {
	expanded!: boolean;
	lines!: Line[];
	sectionType!: SectionType;

	get maxLineNumber(): number {
		return Math.max(
			this.lines[this.lines.length - 1]?.afterLineNumber ?? 0,
			this.lines[this.lines.length - 1]?.beforeLineNumber ?? 0
		);
	}
}

export function parseHunkHeader(header: string | undefined): HunkHeader {
	if (!header) {
		return { beforeStart: 0, beforeLength: 0, afterStart: 0, afterLength: 0 };
	}
	const split = header.split('@@');
	if (split.length < 2) {
		return { beforeStart: 0, beforeLength: 0, afterStart: 0, afterLength: 0 };
	}
	const [before, after] = split[1]?.trim().split(' ') as [string, string];
	const [beforeStart = 0, beforeLength = 0] = before
		.split(',')
		.map((n) => Math.abs(parseInt(n, 10)));
	const [afterStart = 0, afterLength = 0] = after.split(',').map((n) => Math.abs(parseInt(n, 10)));
	return { beforeStart, beforeLength, afterStart, afterLength };
}

export function parseHunkSection(hunk: Hunk | RemoteHunk): HunkSection {
	const lines = hunk.diff.split('\n');
	const header = parseHunkHeader(lines.shift());
	const hunkSection = plainToInstance(HunkSection, {
		hunk: hunk,
		header: header,
		subSections: [],
		hasConflictMarkers:
			hunk.diff.includes('<<<<<<<') &&
			hunk.diff.includes('=======') &&
			hunk.diff.includes('>>>>>>>')
	});

	let currentBeforeLineNumber = header.beforeStart;
	let currentAfterLineNumber = header.afterStart;

	let currentSection: ContentSection | undefined;
	while (lines.length > 0) {
		const line = lines.shift();
		if (!line) break;
		if (line.startsWith('-')) {
			if (!currentSection || currentSection.sectionType !== SectionType.RemovedLines) {
				if (currentSection) hunkSection.subSections.push(currentSection);
				currentSection = plainToInstance(ContentSection, {
					expanded: true,
					lines: [],
					sectionType: SectionType.RemovedLines
				});
			}
			currentSection.lines.push({
				beforeLineNumber: currentBeforeLineNumber,
				afterLineNumber: undefined,
				content: line.slice(1)
			});
			currentBeforeLineNumber++;
		} else if (line.startsWith('+')) {
			if (!currentSection || currentSection.sectionType !== SectionType.AddedLines) {
				if (currentSection) hunkSection.subSections.push(currentSection);
				currentSection = plainToInstance(ContentSection, {
					expanded: true,
					lines: [],
					sectionType: SectionType.AddedLines
				});
			}
			currentSection.lines.push({
				beforeLineNumber: undefined,
				afterLineNumber: currentAfterLineNumber,
				content: line.slice(1)
			});
			currentAfterLineNumber++;
		} else {
			if (!currentSection || currentSection.sectionType !== SectionType.Context) {
				if (currentSection) hunkSection.subSections.push(currentSection);
				currentSection = plainToInstance(ContentSection, {
					expanded: true,
					lines: [],
					sectionType: SectionType.Context
				});
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

export function parseFileSections(file: AnyFile): (ContentSection | HunkSection)[] {
	const hunkSections = file.hunks
		.map(parseHunkSection)
		.filter((hunkSection) => hunkSection !== undefined)
		.sort((a, b) => a.header.beforeStart - b.header.beforeStart);

	const content = file instanceof LocalFile ? file.content : undefined;

	if (!content) return hunkSections;

	const lines = content.split('\n');
	const sections: (ContentSection | HunkSection)[] = [];

	let currentBeforeLineNumber = 1;
	let currentAfterLineNumber = 1;
	let currentContext: ContentSection | undefined = undefined;

	let i = 0;
	while (i < lines.length) {
		if (currentContext === undefined) {
			currentContext = plainToInstance(ContentSection, {
				expanded: true,
				lines: [],
				sectionType: SectionType.Context
			});
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
				content: lines[i] ?? ''
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
