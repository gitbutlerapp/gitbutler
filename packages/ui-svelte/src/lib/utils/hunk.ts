import type { DiffFileLineId } from "$lib/utils/diffParsing";

export function getHunkLineId(rowEncodedId: DiffFileLineId): string {
	return `hunk-line-${rowEncodedId}`;
}

export function generateHunkId(changePath: string, hunkIndex: number): string {
	return `hunk-${changePath}-${hunkIndex}`;
}
