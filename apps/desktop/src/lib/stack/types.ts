import { DetailedCommit, VirtualBranch } from '$lib/vbranches/types';
import { Type } from 'class-transformer';

export class PatchSeries {
	name?: string;
	description?: string;
	upstreamReference?: string;

	@Type(() => DetailedCommit)
	patches!: DetailedCommit[];
	@Type(() => DetailedCommit)
	upstreamPatches!: DetailedCommit[];
}

export class Stack extends VirtualBranch {
	series!: PatchSeries[];
}
