import { CharacterIdMap } from './characterIdMap';
import { diff_match_patch } from 'diff-match-patch';

export function char(text1: string, text2: string, cleanup?: boolean): { 0: number; 1: string }[] {
	const differ = new diff_match_patch();
	const diff = differ.diff_main(text1, text2);
	if (cleanup) {
		differ.diff_cleanupSemantic(diff);
	}
	return diff;
}

export function line(lines1: string[], lines2: string[]): DiffArray {
	const idMap = new CharacterIdMap<string>();
	const text1 = lines1.map((line) => idMap.toChar(line)).join('');
	const text2 = lines2.map((line) => idMap.toChar(line)).join('');

	const diff = char(text1, text2);
	const lineDiff = [];
	for (let i = 0; i < diff.length; i++) {
		const lines = [];
		for (let j = 0; j < diff[i][1].length; j++) {
			lines.push(idMap.fromChar(diff[i][1][j]) || '');
		}

		lineDiff.push({ 0: diff[i][0], 1: lines });
	}
	return lineDiff;
}

export function parse(raw: string): DiffArray {
	const lines = raw.split('\n');

	// skip header lines
	while (isHeaderLine(lines[0])) lines.shift();

	const diff: DiffArray = [];
	for (const line of lines) {
		const gutter = line.substring(0, 1);
		const operation = gutterToOperation(gutter);
		const content = line.substring(1);

		if (diff.length === 0) {
			diff.push([operation, [content]]);
		} else {
			const last = diff[diff.length - 1];
			if (last[0] === operation) {
				last[1].push(content);
			} else {
				diff.push([operation, [content]]);
			}
		}
	}

	return diff;

	function gutterToOperation(gutter: string): Operation {
		switch (gutter) {
			case '+':
				return Operation.Insert;
			case '-':
				return Operation.Delete;
			default:
				return Operation.Equal;
		}
	}

	function isHeaderLine(line: string): boolean {
		const headers = [
			'---',
			'+++',
			'@@',
			'index',
			'diff',
			'rename',
			'similarity',
			'new',
			'deleted',
			'old',
			'copy',
			'dissimilarity'
		];
		for (let i = 0; i < headers.length; i++) {
			if (line.startsWith(headers[i])) {
				return true;
			}
		}
		return false;
	}
}

export enum Operation {
	Equal = 0,
	Insert = 1,
	Delete = -1,
	Edit = 2
}

export type DiffArray = { 0: Operation; 1: string[] }[];
