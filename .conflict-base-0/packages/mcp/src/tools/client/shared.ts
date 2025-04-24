import { z } from 'zod';

export const BaseParamsSchema = z.object({
	project_directory: z.string({ description: 'The absolute path to the project directory' })
});

export function getBranchRef(branchName: string): string {
	if (branchName.startsWith('refs/')) {
		return branchName;
	}
	return `refs/heads/${branchName}`;
}

export type HunkHeader = {
	oldStart: number;
	oldLines: number;
	newStart: number;
	newLines: number;
};

export type DiffSpec = {
	previousPathBytes?: string;
	pathBytes: string;
	hunkHeaders: HunkHeader[];
};
