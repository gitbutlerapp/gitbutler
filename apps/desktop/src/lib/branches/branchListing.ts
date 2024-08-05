// Class transformers will bust a gut if this isn't imported first
import 'reflect-metadata';

import { invoke } from '$lib/backend/ipc';
import { persisted } from '$lib/persisted/persisted';
import { Transform, Type, plainToInstance } from 'class-transformer';
import { get, writable, type Readable, type Writable } from 'svelte/store';

const FILTER_STORAGE_KEY = 'branchListingService-selectedFilter';
export class BranchListingService {
	private selectedFilterPresisted = persisted<BranchListingFilter>(
		{ local: undefined, applied: undefined },
		FILTER_STORAGE_KEY
	);

	private branchListingsWritable = writable<BranchListing[]>([]);

	constructor(private projectId: string) {
		// For now we're not using the selected filter
		this.selectedFilter = {};
		this.refresh();
	}

	async refresh() {
		const listedValues = (await this.list(get(this.selectedFilterPresisted))) || [];
		this.branchListingsWritable.set(listedValues);

		const listedBranchNames = new Set(listedValues.map((entry) => entry.name));

		// Remove branch listings details stores that no longer have cooresponding branches
		for (const key of this.branchListingDetails.keys()) {
			if (!listedBranchNames.has(key)) {
				this.branchListingDetails.delete(key);
			}
		}

		const branchNames = Array.from(this.branchListingDetails.keys());
		this.updateBranchListingDetails(branchNames);
	}

	get selectedFilter(): Readable<BranchListingFilter> {
		return this.selectedFilterPresisted;
	}

	set selectedFilter(value: BranchListingFilter) {
		this.selectedFilterPresisted.set(value);
		this.refresh();
	}

	get branchListings(): Readable<BranchListing[]> {
		return this.branchListingsWritable;
	}

	private async list(filter: BranchListingFilter | undefined = undefined) {
		const entries = await invoke<any[]>('list_branches', { projectId: this.projectId, filter });
		return plainToInstance(BranchListing, entries);
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

		this.updateBranchListingDetails([branchName]);

		return store;
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
