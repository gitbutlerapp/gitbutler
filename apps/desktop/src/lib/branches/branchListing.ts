// Class transformers will bust a gut if this isn't imported first
import 'reflect-metadata';

import { Code, invoke } from '$lib/backend/ipc';
import { debouncedDerive } from '$lib/utils/debounce';
import { msSinceDaysAgo } from '$lib/utils/time';
import { persisted, type Persisted } from '@gitbutler/shared/persisted';
import { Transform, Type, plainToInstance } from 'class-transformer';
import Fuse from 'fuse.js';
import { derived, readable, writable, type Readable, type Writable } from 'svelte/store';
import type { ForgeListingService } from '$lib/forge/interface/forgeListingService';
import type { PullRequest } from '$lib/forge/interface/types';

export class BranchListingService {
	private branchListingsWritable = writable<BranchListing[]>([]);

	constructor(private projectId: string) {
		this.refresh();
	}

	get branchListings(): Readable<BranchListing[]> {
		return this.branchListingsWritable;
	}

	async refresh() {
		const listedValues = (await this.list({})) || [];
		this.branchListingsWritable.set(listedValues);

		const displayedBranchListingDetails = Array.from(this.branchListingDetails.keys());
		this.updateBranchListingDetails(displayedBranchListingDetails);
	}

	private async list(filter: BranchListingFilter | undefined = undefined) {
		try {
			const entries = await invoke<any[]>('list_branches', { projectId: this.projectId, filter });
			return plainToInstance(BranchListing, entries);
		} catch (error: any) {
			if (error.code === Code.DefaultTargetNotFound) {
				// Swallow this error since user should be taken to project setup page
				return undefined;
			}
		}
	}

	private branchListingDetails = new Map<string, Writable<BranchListingDetails | undefined>>();
	/**
	 * Fetches the details for a particular branch.
	 *
	 * A store is returned so the result can be refreshed when needed
	 */
	getBranchListingDetails(branchName: string): Readable<BranchListingDetails | undefined> {
		if (this.branchListingDetails.has(branchName)) {
			return this.branchListingDetails.get(branchName)!;
		}

		const store = writable<BranchListingDetails | undefined>();
		this.branchListingDetails.set(branchName, store);

		this.updateBranchListing(branchName);

		return store;
	}

	/**
	 * Refresh the information for a particular branch.
	 *
	 * Will only fetch the information if the branch is already being tracked.
	 */
	refreshBranchListingDetails(branchName: string) {
		if (!this.branchListingDetails.has(branchName)) {
			return;
		}
		this.updateBranchListing(branchName);
	}

	private branchFetchQueue: string[] = [];
	private updateBranchListingTimeout: ReturnType<typeof setTimeout> | undefined;
	// Debounces multiple update calls
	private async updateBranchListing(branchName: string) {
		this.branchFetchQueue.push(branchName);

		clearTimeout(this.updateBranchListingTimeout);
		this.updateBranchListingTimeout = setTimeout(
			(() => {
				this.updateBranchListingDetails(this.branchFetchQueue);
				this.branchFetchQueue = [];
			}).bind(this),
			50
		);
	}

	private async updateBranchListingDetails(branchNames: string[]) {
		try {
			const plainDetails = await invoke<unknown[]>('get_branch_listing_details', {
				projectId: this.projectId,
				branchNames
			});

			const branchListingDetails = plainToInstance(BranchListingDetails, plainDetails);

			branchListingDetails.forEach((branchListingDetails) => {
				let store = this.branchListingDetails.get(branchListingDetails.name);

				store ??= writable();

				store.set(branchListingDetails);
			});
		} catch (error: any) {
			if (error.code === Code.DefaultTargetNotFound) {
				// Swallow this error since user should be taken to project setup page
				return;
			}

			throw error;
		}
	}
}

export type GroupedSidebarEntries = Record<
	'applied' | 'today' | 'yesterday' | 'lastWeek' | 'older',
	SidebarEntrySubject[]
>;

export class CombinedBranchListingService {
	private pullRequests: Readable<PullRequest[]>;
	selectedOption: Persisted<'all' | 'pullRequest' | 'local'>;

	// Used when deriving the search results and for finding the total number
	// of listings
	combinedSidebarEntries: Readable<SidebarEntrySubject[]>;
	// Contains entries grouped by date for the unsearched sidebar entries
	groupedSidebarEntries: Readable<GroupedSidebarEntries>;
	// Whether or not to show the pull request tab in the sidebar
	pullRequestsListed: Readable<boolean>;

	constructor(
		branchListingService: BranchListingService,
		forgeListingService: Readable<ForgeListingService | undefined>,
		projectId: string
	) {
		this.selectedOption = persisted<'all' | 'pullRequest' | 'local'>(
			'all',
			`branches-selectedOption-${projectId}`
		);

		// Get a readable store of pull requeests
		this.pullRequests = readable([] as PullRequest[], (set) => {
			const unsubscribeListingService = forgeListingService.subscribe((forgeListingService) => {
				if (!forgeListingService) return;

				const unsubscribePullRequests = forgeListingService.prs.subscribe((prs) => {
					set(prs);
				});

				return unsubscribePullRequests;
			});

			return unsubscribeListingService;
		});

		// Whether or not to show the pull request tab in the sidebar
		this.pullRequestsListed = derived(
			forgeListingService,
			(forgeListingService) => {
				return !!forgeListingService;
			},
			false
		);

		// Derive the combined sidebar entries
		this.combinedSidebarEntries = debouncedDerive(
			[this.pullRequests, branchListingService.branchListings, this.selectedOption],
			([pullRequests, branchListings, selectedOption]) => {
				// Find the pull requests that don't have cooresponding local branches,
				// remote branches, virutal branches, or stack branch heads.
				// Then map the pull requests into the SidebarEntrySubject type
				const pullRequestSubjects: SidebarEntrySubject[] = pullRequests
					.filter(
						(pullRequests) =>
							!branchListings.some((branchListing) =>
								branchListing.containsPullRequestBranch(pullRequests.sourceBranch)
							)
					)
					.map((pullRequests) => ({ type: 'pullRequest', subject: pullRequests }));

				// Map the raw branch listing classes into the
				// SidebarEntrySubject type
				const branchListingSubjects: SidebarEntrySubject[] = branchListings.map(
					(branchListing) => ({
						type: 'branchListing',
						subject: branchListing
					})
				);

				// Sort the combined entries by when htey were last updated
				const output = [...pullRequestSubjects, ...branchListingSubjects];

				output.sort((a, b) => {
					const timeDifference =
						getEntryUpdatedDate(b).getTime() - getEntryUpdatedDate(a).getTime();
					if (timeDifference !== 0) {
						return timeDifference;
					}

					return getEntryName(a).localeCompare(getEntryName(b));
				});

				// Filter by the currently selected tab in the frontend
				const filtered = this.filterSidebarEntries(pullRequests, selectedOption, output);

				return filtered;
			},
			[] as SidebarEntrySubject[],
			50
		);

		// Create the entries which are grouped date-wise
		this.groupedSidebarEntries = derived(this.combinedSidebarEntries, (combinedSidebarEntries) => {
			const groupings = this.groupBranches(combinedSidebarEntries);
			return groupings;
		});
	}

	search(searchTerm: Readable<string | undefined>) {
		const engine = derived(this.combinedSidebarEntries, (combinedSidebarEntries) => {
			return new Fuse(combinedSidebarEntries, {
				keys: [
					// Subject is branch listing
					'subject.name',
					'subject.lastCommiter.email',
					'subject.lastCommiter.name',
					// Subject is pull request
					'subject.title',
					'subject.author.email',
					'subject.author.name'
				],
				threshold: 0.3, // 0 is the strictest.
				ignoreLocation: true,
				isCaseSensitive: false,
				sortFn: (a, b) => {
					// Sort results by when the item was last modified.
					const dateA = (a.item.modifiedAt ?? a.item.updatedAt) as Date | undefined;
					const dateB = (b.item.modifiedAt ?? b.item.updatedAt) as Date | undefined;
					if (dateA !== undefined && dateB !== undefined && dateA !== dateB) {
						return dateA < dateB ? -1 : 1;
					}
					// If there are no dates or they're the same, sort by score
					return a.score < b.score ? -1 : 1;
				}
			});
		});

		return derived(
			[searchTerm, engine, this.combinedSidebarEntries],
			([searchTerm, engine, combinedSidebarEntries]) => {
				if (!searchTerm) return combinedSidebarEntries;
				const results = engine.search(searchTerm);
				return results.map((result) => result.item);
			},
			[] as SidebarEntrySubject[]
		);
	}

	private groupBranches(branches: SidebarEntrySubject[]) {
		const grouped: GroupedSidebarEntries = {
			applied: [],
			today: [],
			yesterday: [],
			lastWeek: [],
			older: []
		};

		const now = Date.now();

		branches.forEach((b) => {
			if (!getEntryUpdatedDate(b)) {
				grouped.older.push(b);
				return;
			}

			const msSinceLastCommit = now - getEntryUpdatedDate(b).getTime();

			if (getEntryWorkspaceStatus(b)) {
				grouped.applied.push(b);
			} else if (msSinceLastCommit < msSinceDaysAgo(1)) {
				grouped.today.push(b);
			} else if (msSinceLastCommit < msSinceDaysAgo(2)) {
				grouped.yesterday.push(b);
			} else if (msSinceLastCommit < msSinceDaysAgo(7)) {
				grouped.lastWeek.push(b);
			} else {
				grouped.older.push(b);
			}
		});

		return grouped;
	}

	private filterSidebarEntries(
		pullRequests: PullRequest[],
		selectedOption: string,
		sidebarEntries: SidebarEntrySubject[]
	): SidebarEntrySubject[] {
		switch (selectedOption) {
			case 'pullRequest': {
				return sidebarEntries.filter(
					(sidebarEntry) =>
						sidebarEntry.type === 'pullRequest' ||
						pullRequests.some((pullRequest) =>
							sidebarEntry.subject.containsPullRequestBranch(pullRequest.sourceBranch)
						)
				);
			}
			case 'local': {
				return sidebarEntries.filter(
					(sidebarEntry) =>
						sidebarEntry.type === 'branchListing' &&
						(sidebarEntry.subject.hasLocal || sidebarEntry.subject.virtualBranch)
				);
			}
			default: {
				return sidebarEntries;
			}
		}
	}
}

/** A filter that can be applied to the branch listing */
export interface BranchListingFilter {
	/**
	 * If the value is true, the listing will only include branches that have a local branch or virtual branch
	 * If the value is false, the listing will include only branches that do not have a local branch or virtual branch
	 */
	local?: boolean;
	/**
	 * If the value is true, the listing will only include branches that are applied in the workspace.
	 * If the value is false, the listing will only include branches that are not applied in the workspace.
	 */
	applied?: boolean;
}

/**
 * Represents a branch that exists for the repository
 * This also combines the concept of a remote, local and virtual branch in order to provide a unified interface for the UI
 * Branch entry is not meant to contain all of the data a branch can have (e.g. full commit history, all files and diffs, etc.).
 * It is intended a summary that can be quickly retrieved and displayed in the UI.
 * For more detailed information, each branch can be queried individually for it's `BranchData`.
 */
export class BranchListing {
	/** The name of the branch (e.g. `main`, `feature/branch`), excluding the remote name */
	name!: string;
	/**
	 * This is a list of remote that this branch can be found on (e.g. `origin`, `upstream` etc.).
	 * If this branch is a local branch, this list will be empty.
	 */
	remotes!: string[];
	/** The branch may or may not have a virtual branch associated with it */
	@Type(() => VirtualBranchReference)
	virtualBranch?: VirtualBranchReference | undefined;
	/**
	 * Timestamp in milliseconds since the branch was last updated.
	 * This includes any commits, uncommited changes or even updates to the branch metadata (e.g. renaming).
	 */
	@Transform((obj) => new Date(obj.value))
	updatedAt!: Date;
	/** The person who commited the head commit */
	@Type(() => Author)
	lastCommiter!: Author;
	/** Whether or not there is a local branch as part of the grouping */
	hasLocal!: boolean;

	containsPullRequestBranch(sourceBranch: string): boolean {
		if (sourceBranch === this.name) return true;
		if (this.virtualBranch?.stackBranches.includes(sourceBranch)) return true;

		return false;
	}
}

/** Represents a reference to an associated virtual branch */
export class VirtualBranchReference {
	/** A non-normalized name of the branch, set by the user */
	givenName!: string;
	/** Virtual Branch UUID identifier */
	id!: string;
	/** Determines if the virtual branch is applied in the workspace */
	inWorkspace!: boolean;
	/**
   List of branch names that are part of the stack
   Ordered from newest to oldest (the most recent branch is first in the list)
    */
	stackBranches!: string[];
	/** Pull Request numbes by branch name associated with the stack */
	pullRequests!: Map<string, number>;
}

/** Represents a "commit author" or "signature", based on the data from ther git history */
export class Author {
	/** The name of the author as configured in the git config */
	name?: string | undefined;
	/** The email of the author as configured in the git config */
	email?: string | undefined;
	/** The gravatar id of the author */
	gravatarUrl?: string | undefined;
}

/** Represents a fat struct with all the data associated with a branch */
export class BranchListingDetails {
	/** The name of the branch (e.g. `main`, `feature/branch`), excluding the remote name */
	name!: string;
	/**
	 * The number of lines added within the branch
	 * Since the virtual branch, local branch and the remote one can have different number of lines removed,
	 * the value from the virtual branch (if present) takes the highest precedence,
	 * followed by the local branch and then the remote branches (taking the max if there are multiple).
	 * If this branch has a virutal branch, lines_added does NOT include the uncommitted lines.
	 */
	linesAdded!: number;
	/**
	 * The number of lines removed within the branch
	 * Since the virtual branch, local branch and the remote one can have different number of lines removed,
	 * the value from the virtual branch (if present) takes the highest precedence,
	 * followed by the local branch and then the remote branches (taking the max if there are multiple)
	 * If this branch has a virutal branch, lines_removed does NOT include the uncommitted lines.
	 */
	linesRemoved!: number;
	/**
	 * The number of files that were modified within the branch
	 * Since the virtual branch, local branch and the remote one can have different number files modified,
	 * the value from the virtual branch (if present) takes the highest precedence,
	 * followed by the local branch and then the remote branches (taking the max if there are multiple)
	 */
	numberOfFiles!: number;
	/**
	 * The number of commits associated with a branch
	 * Since the virtual branch, local branch and the remote one can have different number of commits,
	 * the value from the virtual branch (if present) takes the highest precedence,
	 * followed by the local branch and then the remote branches (taking the max if there are multiple)
	 */
	numberOfCommits!: number;
	/**
	 * A list of authors that have contributes commits to this branch.
	 * In the case of multiple remote tracking branches, it takes the full list of unique authors.
	 */
	@Type(() => Author)
	authors!: Author[];
}

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
