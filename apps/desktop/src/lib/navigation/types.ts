import type { BranchListing } from '$lib/branches/branchListing';
import type { PullRequest } from '$lib/forge/interface/types';

export type SidebarEntrySubject =
	| {
			type: 'pullRequest';
			subject: PullRequest;
	  }
	| {
			type: 'branchListing';
			subject: BranchListing;
	  };

export function getEntryUpdatedDate(entry: SidebarEntrySubject) {
	return entry.type === 'branchListing' ? entry.subject.updatedAt : entry.subject.modifiedAt;
}

export function getEntryName(entry: SidebarEntrySubject) {
	return entry.type === 'branchListing' ? entry.subject.name : entry.subject.title;
}

export function getEntryWorkspaceStatus(entry: SidebarEntrySubject) {
	return entry.type === 'branchListing' ? entry.subject.virtualBranch?.inWorkspace : undefined;
}
