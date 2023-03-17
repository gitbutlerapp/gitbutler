import { listFiles } from '$lib/sessions';
import { list as listDeltas } from '$lib/deltas';
import type { SearchResult } from '$lib';
import { structuredPatch } from 'diff';
import type { Delta } from '$lib/deltas';
import { Operation } from '$lib/deltas';
import type { ProcessedSearchResult, ProcessedSearchRestultHunk } from '.';

export const processSearchResult = async (
	searchResult: SearchResult,
	query: string
): Promise<ProcessedSearchResult> => {
	const [files, deltas] = await Promise.all([
		listFiles({
			projectId: searchResult.projectId,
			sessionId: searchResult.sessionId,
			paths: [searchResult.filePath]
		}),
		listDeltas({
			projectId: searchResult.projectId,
			sessionId: searchResult.sessionId,
			paths: [searchResult.filePath]
		})
	]);
	const hunks = getDiffHunksWithSearchTerm(
		files[searchResult.filePath],
		deltas[searchResult.filePath],
		searchResult.index,
		query
	);

	const processedHunks = [];
	for (let i = 0; i < hunks.length; i++) {
		const processedHunk: ProcessedSearchRestultHunk = {
			lines: processHunkLines(hunks[i].lines, hunks[i].newStart, query)
		};
		processedHunks.push(processedHunk);
	}

	const processedSearchResult: ProcessedSearchResult = {
		searchResult: searchResult,
		hunks: processedHunks,
		timestamp: new Date(deltas[searchResult.filePath][searchResult.index].timestampMs)
	};
	return processedSearchResult;
};

const applyDeltas = (text: string, deltas: Delta[]) => {
	const operations = deltas.flatMap((delta) => delta.operations);

	operations.forEach((operation) => {
		if (Operation.isInsert(operation)) {
			text =
				text.slice(0, operation.insert[0]) + operation.insert[1] + text.slice(operation.insert[0]);
		} else if (Operation.isDelete(operation)) {
			text =
				text.slice(0, operation.delete[0]) + text.slice(operation.delete[0] + operation.delete[1]);
		}
	});
	return text;
};

const getDiffHunksWithSearchTerm = (
	original: string,
	deltas: Delta[],
	idx: number,
	query: string
) => {
	if (!original) return [];
	return structuredPatch(
		'file',
		'file',
		applyDeltas(original, deltas.slice(0, idx)),
		applyDeltas(original, [deltas[idx]]),
		'header',
		'header',
		{ context: 1 }
	).hunks.filter((hunk) => hunk.lines.some((l) => l.includes(query)));
};

const processHunkLines = (lines: string[], newStart: number, query: string) => {
	const outLines = [];

	let lineNumber = newStart;
	for (let i = 0; i < lines.length; i++) {
		const line = lines[i];

		let contentBeforeHit = '';
		let querySubstring = '';
		let contentAfterHit = '';
		if (!line.includes(query)) {
			contentBeforeHit = line.slice(1);
		} else {
			const firstCharIndex = line.indexOf(query);
			const lastCharIndex = firstCharIndex + query.length - 1;
			contentBeforeHit = line.slice(1, firstCharIndex);
			querySubstring = line.slice(firstCharIndex, lastCharIndex + 1);
			contentAfterHit = line.slice(lastCharIndex + 1);
		}

		outLines.push({
			hidden: false,
			contentBeforeHit: contentBeforeHit,
			contentAtHit: querySubstring,
			contentAfterHit: contentAfterHit,
			operation: line.startsWith('+') ? 'add' : line.startsWith('-') ? 'remove' : 'unmodified',
			lineNumber: !line.startsWith('-') ? lineNumber : undefined,
			hasKeyword: line.includes(query)
		});

		if (!line.startsWith('-')) {
			lineNumber++;
		}
	}

	const out = [];
	for (let i = 0; i < outLines.length; i++) {
		const prevLine = outLines[i - 1];
		const nextLine = outLines[i + 1];
		const line = outLines[i];
		if (line.hasKeyword) {
			out.push(line);
		} else if (nextLine && nextLine.hasKeyword) {
			// One line of context before the relevant line
			out.push(line);
		} else if (prevLine && prevLine.hasKeyword) {
			// One line of context after the relevant line
			out.push(line);
		} else {
			line.hidden = true;
			out.push(line);
		}
	}
	return out;
};
