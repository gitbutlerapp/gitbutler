interface DiffChunk {
	header: string;
	lines: string[];
}

function getDiffChunks(diff: string): DiffChunk[] {
	const lines = diff.split('\n');
	const chunks: DiffChunk[] = [];
	let chunk: DiffChunk | null = null;
	let line: string | null = null;

	for (let i = 0; i < lines.length; i++) {
		line = lines[i];

		if (line.startsWith('diff')) {
			if (chunk) {
				chunks.push(chunk);
			}

			chunk = {
				header: line,
				lines: []
			};

			continue;
		}

		if (line.startsWith('@@')) {
			if (chunk) {
				chunks.push(chunk);
			}

			chunk = {
				header: line,
				lines: []
			};

			continue;
		}

		if (line.startsWith('---') || line.startsWith('+++')) {
			// Ignore
			continue;
		}

		if (line.startsWith('index')) {
			// Ignore
			continue;
		}

		if (chunk) {
			chunk.lines.push(line);
		}
	}

	if (chunk) {
		chunks.push(chunk);
	}

	return chunks;
}

interface ParsedDiffHeader {
	oldStart: number;
	oldLength: number;
	newStart: number;
	newLength: number;
}

function parseHeader(rawHeader: string): ParsedDiffHeader {
	const parts = rawHeader.split(' ');
	const oldStart = parts[0].split(',')[0].slice(1);
	const oldLength = parts[0].split(',')[1];
	const newStart = parts[1].split(',')[0];
	const newLength = parts[1].split(',')[1];

	return {
		oldStart: parseInt(oldStart, 10),
		oldLength: parseInt(oldLength, 10),
		newStart: parseInt(newStart, 10),
		newLength: parseInt(newLength, 10)
	};
}

export type DiffLineType = 'add' | 'remove' | 'context';

interface ParsedDiffLine {
	type: DiffLineType;
	line: string;
}

interface ParsedDiffHunk {
	header: ParsedDiffHeader;
	lines: ParsedDiffLine[];
}
export function parseDiff(diff: string | undefined): ParsedDiffHunk[] {
	if (!diff) {
		return [];
	}

	const chunks = getDiffChunks(diff);
	const parsedChunks: ParsedDiffHunk[] = [];
	for (const chunk of chunks) {
		if (!chunk.header.startsWith('@@')) {
			continue;
		}

		const rawHeader = chunk.header.split('@@')[1].trim();
		const parsedHeader = parseHeader(rawHeader);
		const lines: ParsedDiffLine[] = [];

		for (const rawLine of chunk.lines) {
			const type = rawLine[0];
			const line = rawLine.slice(1);

			lines.push({
				type: type === '+' ? 'add' : type === '-' ? 'remove' : 'context',
				line
			});
		}

		parsedChunks.push({
			header: parsedHeader,
			lines
		});
	}

	return parsedChunks;
}
