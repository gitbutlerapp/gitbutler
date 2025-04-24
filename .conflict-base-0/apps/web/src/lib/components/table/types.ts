import { type CommitStatusType } from '@gitbutler/ui/CommitStatusBadge.svelte';
import type { Branch } from '@gitbutler/shared/branches/types';

export type AvatarsType = {
	srcUrl: string;
	name: string;
};

export type ChangesType = {
	additions: number;
	deletions: number;
};

export type ColumnTypes = {
	status: CommitStatusType;
	version: string;
	changes: ChangesType;
	title: string;
	string: string;
	comments: string;
	number: number;
	date: Date;
	commitGraph: { branch: Branch; projectSlug: string; ownerSlug: string };
	avatars: AvatarsType[];
	reviewers: {
		approvers: AvatarsType[];
		rejectors: AvatarsType[];
	};
	position: string;
};
