import { invoke } from '$lib/backend/ipc';
import { plainToInstance } from 'class-transformer';

export class BranchListingService {
	constructor(private projectId: string) {}
	async list(filter: BranchListingFilter | undefined = undefined) {
		try {
			const branches = plainToInstance(
				BranchListing,
				await invoke<any[]>('list_branches', { projectId: this.projectId, filter })
			);
			console.log(branches);
			return branches;
		} catch (err: any) {
			console.error(err);
		}
	}
}

/// A filter that can be applied to the branch listing
export class BranchListingFilter {
    /// If the value is true, the listing will only include branches that have the same author as the current user.
    /// If the value is false, the listing will include only branches that are not created by the user.
    ownBranches?: boolean;
    /// If the value is true, the listing will only include branches that are applied in the workspace.
    /// If the value is false, the listing will only include branches that are not applied in the workspace.
    applied?: boolean;
}

/// Represents a branch that exists for the repository
/// This also combines the concept of a remote, local and virtual branch in order to provide a unified interface for the UI
/// Branch entry is not meant to contain all of the data a branch can have (e.g. full commit history, all files and diffs, etc.).
/// It is intended a summary that can be quickly retrieved and displayed in the UI.
/// For more detailed information, each branch can be queried individually for it's `BranchData`.
export class BranchListing {
	/// The name of the branch (e.g. `main`, `feature/branch`), excluding the remote name
	name!: string;
	/// This is a list of remote that this branch can be found on (e.g. `origin`, `upstream` etc.).
	/// If this branch is a local branch, this list will be empty.
	remotes!: string[];
	/// The branch may or may not have a virtual branch associated with it
	virtualBranch?: VirtualBranchReference | undefined;
	/// The number of commits associated with a branch
	/// Since the virtual branch, local branch and the remote one can have different number of commits,
	/// the value from the virtual branch (if present) takes the highest precedence,
	/// followed by the local branch and then the remote branches (taking the max if there are multiple)
	numberOfCommits!: number;
	/// Timestamp in milliseconds since the branch was last updated.
	/// This includes any commits, uncommited changes or even updates to the branch metadata (e.g. renaming).
	updatedAt!: number;
	/// A list of authors that have contributes commits to this branch.
	/// In the case of multiple remote tracking branches, it takes the full list of unique authors.
	authors!: Author[];
	/// Determines if the branch is considered one created by the user
	/// A branch is considered created by the user if they were the author of the first commit in the branch.
	ownBranch!: boolean;
}

/// Represents a reference to an associated virtual branch
export class VirtualBranchReference {
	/// A non-normalized name of the branch, set by the user
	givenName!: string;
	/// Virtual Branch UUID identifier
	id!: string;
	/// Determines if the virtual branch is applied in the workspace
	inWorkspace!: boolean;
}

/// Represents a "commit author" or "signature", based on the data from ther git history
export class Author {
	/// The name of the author as configured in the git config
	name?: string | undefined;
	/// The email of the author as configured in the git config
	email?: string | undefined;
}
