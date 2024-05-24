import type { Commit, RemoteCommit } from '$lib/vbranches/types';

export function getAvatarTooltip(commit: Commit | RemoteCommit | undefined): string | undefined {
	if (commit) {
		return [commit.author.name, commit.descriptionTitle, commit.id.substring(0, 6)].join('\n');
	}
}
