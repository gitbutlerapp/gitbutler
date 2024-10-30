// Class transformers will bust a gut if this isn't imported first
import 'reflect-metadata';

import { Code, invoke } from '$lib/backend/ipc';
import {
	getEntryName,
	getEntryUpdatedDate,
	getEntryWorkspaceStatus,
	type SidebarEntrySubject
} from '$lib/navigation/types';
import { debouncedDerive } from '$lib/utils/debounce';
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

	private accumulatedBranchListings: string[] = [];
	private updateBranchListingTimeout: ReturnType<typeof setTimeout> | undefined;
	// Accumulates multiple update calls
	private async updateBranchListing(branchName: string) {
		this.accumulatedBranchListings.push(branchName);

		clearTimeout(this.updateBranchListingTimeout);
		this.updateBranchListingTimeout = setTimeout(
			(() => {
				this.updateBranchListingDetails(this.accumulatedBranchListings);
				this.accumulatedBranchListings = [];
			}).bind(this),
			50
		);
	}

	private async updateBranchListingDetails(branchNames: string[]) {
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
	}
}

const oneDay = 1000 * 60 * 60 * 24;
export type GroupedSidebarEntries = Record<
	'applied' | 'today' | 'yesterday' | 'lastWeek' | 'older',
	SidebarEntrySubject[]
>;

export class CombinedBranchListingService {
	private pullRequests: Readable<PullRequest[]>;
	selectedOption: Persisted<'all' | 'pullRequest' | 'local'>;

	combinedSidebarEntries: Readable<SidebarEntrySubject[]>;
	groupedSidebarEntries: Readable<GroupedSidebarEntries>;
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

		this.pullRequestsListed = derived(
			forgeListingService,
			(forgeListingService) => {
				return !!forgeListingService;
			},
			false
		);

		const branchListingsByName = derived(branchListingService.branchListings, (branchListings) => {
			const set = new Set<string>(branchListings.map((branchListing) => branchListing.name));
			return set;
		});

		this.combinedSidebarEntries = debouncedDerive(
			[
				branchListingsByName,
				this.pullRequests,
				branchListingService.branchListings,
				this.selectedOption
			],
			([branchListingsByName, pullRequests, branchListings, selectedOption]) => {
				const pullRequestSubjects: SidebarEntrySubject[] = pullRequests
					.filter((pullRequests) => !branchListingsByName.has(pullRequests.sourceBranch))
					.map((pullRequests) => ({ type: 'pullRequest', subject: pullRequests }));

				const branchListingSubjects: SidebarEntrySubject[] = branchListings.map(
					(branchListing) => ({
						type: 'branchListing',
						subject: branchListing
					})
				);

				const output = [...pullRequestSubjects, ...branchListingSubjects];

				output.sort((a, b) => {
					const timeDifference =
						getEntryUpdatedDate(b).getTime() - getEntryUpdatedDate(a).getTime();
					if (timeDifference !== 0) {
						return timeDifference;
					}

					return getEntryName(a).localeCompare(getEntryName(b));
				});

				const filtered = this.filterSidebarEntries(pullRequests, selectedOption, output);

				return filtered;
			},
			[] as SidebarEntrySubject[],
			50
		);

		this.groupedSidebarEntries = derived(this.combinedSidebarEntries, (combinedSidebarEntries) => {
			const groupings = this.groupBranches(combinedSidebarEntries);
			return groupings;
		});
	}

	search(searchTerm: Readable<string | undefined>) {
		return derived(
			[searchTerm, this.combinedSidebarEntries],
			([searchTerm, combinedSidebarEntries]) => {
				if (!searchTerm) return [];

				const fuse = new Fuse(combinedSidebarEntries, {
					keys: ['subject.name', 'subject.title']
				});

				return fuse
					.search(searchTerm)
					.slice(0, 100)
					.map((searchResult) => searchResult.item);
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
			} else if (msSinceLastCommit < oneDay) {
				grouped.today.push(b);
			} else if (msSinceLastCommit < 2 * oneDay) {
				grouped.yesterday.push(b);
			} else if (msSinceLastCommit < 7 * oneDay) {
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
						pullRequests.some(
							(pullRequest) => pullRequest.sourceBranch === sidebarEntry.subject.name
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
}

/** Represents a reference to an associated virtual branch */
export class VirtualBranchReference {
	/** A non-normalized name of the branch, set by the user */
	givenName!: string;
	/** Virtual Branch UUID identifier */
	id!: string;
	/** Determines if the virtual branch is applied in the workspace */
	inWorkspace!: boolean;
}

/** Represents a "commit author" or "signature", based on the data from ther git history */
export class Author {
	/** The name of the author as configured in the git config */
	name?: string | undefined;
	/** The email of the author as configured in the git config */
	email?: string | undefined;
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
