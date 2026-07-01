import { SectionType, type ContentSection, type Line } from "@gitbutler/ui/utils/diffParsing";
import type { DiffPatch } from "@gitbutler/shared/chat/types";

function getSectionType(line: DiffPatch): SectionType {
	switch (line.type) {
		case "added":
			return SectionType.AddedLines;
		case "removed":
			return SectionType.RemovedLines;
		case "context":
			return SectionType.Context;
	}
}

function cleanDiffLine(line: string, sectionType: SectionType): string {
	if (sectionType === SectionType.AddedLines && line.startsWith("+")) {
		return line.slice(1);
	}
	if (sectionType === SectionType.RemovedLines && line.startsWith("-")) {
		return line.slice(1);
	}
	return line;
}

export function parseDiffPatchToContentSection(
	diffPatchArray: DiffPatch[] | undefined,
): ContentSection[] {
	if (!diffPatchArray || diffPatchArray.length === 0) {
		return [];
	}

	const content: ContentSection[] = [];
	let lines: Line[] = [];
	let lastSectionType: SectionType | undefined = undefined;
	for (const line of diffPatchArray) {
		const currentType = getSectionType(line);
		if (lastSectionType === undefined) {
			lastSectionType = currentType;
		} else if (lastSectionType !== currentType) {
			content.push({
				sectionType: lastSectionType,
				lines,
			});
			lines = [];
			lastSectionType = currentType;
		}

		lines.push({
			content: cleanDiffLine(line.line, currentType),
			beforeLineNumber: line.left,
			afterLineNumber: line.right,
		});
	}

	if (lines.length > 0 && lastSectionType !== undefined) {
		content.push({
			sectionType: lastSectionType,
			lines,
		});
	}

	return content;
}

export function parseDiffPatchToDiffString(
	diffPatchArray: DiffPatch[] | undefined,
	side: "before" | "after",
): string | undefined {
	if (!diffPatchArray || diffPatchArray.length === 0) {
		return undefined;
	}

	return diffPatchArray
		.map((line) => {
			switch (line.type) {
				case "added":
					return side === "after" ? cleanDiffLine(line.line, SectionType.AddedLines) : undefined;
				case "removed":
					return side === "before" ? cleanDiffLine(line.line, SectionType.RemovedLines) : undefined;
				case "context":
					return line.line;
			}
		})
		.filter((line) => line !== undefined)
		.join("\n");
}
