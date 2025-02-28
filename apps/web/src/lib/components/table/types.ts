import { type CommitStatusType } from '@gitbutler/ui/CommitStatusBadge.svelte';

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
	changes: ChangesType;
	title: string;
	string: string;
	comments: string;
	date: Date;
	avatars: Array<AvatarsType>;
	reviewers: {
		approvers: Array<AvatarsType>;
		rejectors: Array<AvatarsType>;
	};
};
