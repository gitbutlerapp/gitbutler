import { CharacterIdMap } from './characterIdMap';
import { diff_match_patch } from 'diff-match-patch';

export const charDiff = (
    text1: string,
    text2: string,
    cleanup?: boolean
): { 0: number; 1: string }[] => {
    const differ = new diff_match_patch();
    const diff = differ.diff_main(text1, text2);
    if (cleanup) {
        differ.diff_cleanupSemantic(diff);
    }
    return diff;
};

export const lineDiff = (lines1: string[], lines2: string[]): DiffArray => {
    const idMap = new CharacterIdMap<string>();
    const text1 = lines1.map((line) => idMap.toChar(line)).join('');
    const text2 = lines2.map((line) => idMap.toChar(line)).join('');

    const diff = charDiff(text1, text2);
    const lineDiff = [];
    for (let i = 0; i < diff.length; i++) {
        const lines = [];
        for (let j = 0; j < diff[i][1].length; j++) {
            lines.push(idMap.fromChar(diff[i][1][j]) || '');
        }

        lineDiff.push({ 0: diff[i][0], 1: lines });
    }
    return lineDiff;
};

// TODO(crbug.com/1167717): Make this a const enum again
// eslint-disable-next-line rulesdir/const_enum
export enum Operation {
    Equal = 0,
    Insert = 1,
    Delete = -1,
    Edit = 2
}

export type DiffArray = { 0: Operation; 1: string[] }[];
