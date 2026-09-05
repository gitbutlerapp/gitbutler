import {
	invalidatesList,
	invalidatesType,
	providesList,
	providesType,
	ReduxTag,
} from "$lib/state/tags";
import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type { ForgeProvider, RemoteBranchInfo } from "$lib/baseBranch/baseBranch";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type { BaseBranch, WorkspaceFetchStatus } from "@gitbutler/but-sdk";
import type { BranchListing, BranchListingDetails } from "@gitbutler/but-sdk";

export function buildBranchEndpoints(build: BackendEndpointBuilder) {
	return {
		// ── Base Branch ─────────────────────────────────────────────
		forgeProvider: build.query<ForgeProvider | null, { projectId: string }>({
			extraOptions: { command: "forge_provider" },
			query: (args) => args,
			providesTags: [providesType(ReduxTag.ForgeProvider)],
		}),
		baseBranch: build.query<BaseBranch | undefined, { projectId: string }>({
			extraOptions: { command: "get_base_branch_data" },
			query: (args) => args,
			providesTags: [providesType(ReduxTag.BaseBranchData)],
		}),
		workspaceFetchStatus: build.query<WorkspaceFetchStatus, { projectId: string }>({
			extraOptions: { command: "workspace_fetch_status" },
			query: (args) => args,
			providesTags: [providesType(ReduxTag.WorkspaceFetchStatus)],
		}),
		workspaceFetchFromRemotes: build.mutation<void, { projectId: string; action?: string }>({
			// No actionName: auto-fetch runs this on a timer, and a named event
			// would sidestep the per-command sampling of tauri_command events.
			extraOptions: { command: "workspace_fetch_from_remotes" },
			query: ({ projectId, action }) => ({
				projectId,
				action: action ?? "auto",
			}),
			invalidatesTags: [
				invalidatesType(ReduxTag.WorkspaceFetchStatus),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
			],
		}),
		setTarget: build.mutation<
			BaseBranch,
			{ projectId: string; branch: string; pushRemote?: string; stashUncommitted?: boolean }
		>({
			extraOptions: { command: "set_base_branch" },
			query: (args) => args,
			invalidatesTags: [
				invalidatesType(ReduxTag.ForgeProvider),
				// The review listing follows the target remote: a stopped
				// (unrecognized-forge) listing must retry once it changes.
				invalidatesList(ReduxTag.PullRequests),
				invalidatesType(ReduxTag.BaseBranchData),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
			],
		}),
		// Like setTarget, but only writes project metadata: the user stays on the
		// current branch instead of being moved into the GitButler workspace.
		setTargetRef: build.mutation<
			void,
			{ projectId: string; targetRef: string; pushRemote?: string }
		>({
			extraOptions: { command: "set_target_ref_and_init_project" },
			query: (args) => args,
			invalidatesTags: [
				invalidatesType(ReduxTag.ForgeProvider),
				invalidatesList(ReduxTag.PullRequests),
				invalidatesType(ReduxTag.BaseBranchData),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
				// No branch is checked out, so no `git/head` event refreshes the
				// operating mode - invalidate it explicitly.
				invalidatesList(ReduxTag.HeadMetadata),
			],
		}),
		switchBackToWorkspace: build.mutation<BaseBranch, { projectId: string }>({
			extraOptions: { command: "switch_back_to_workspace", actionName: "Switch Back to Workspace" },
			query: (args) => args,
			invalidatesTags: [
				invalidatesType(ReduxTag.ForgeProvider),
				invalidatesType(ReduxTag.BaseBranchData),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
			],
		}),
		remoteBranches: build.query<RemoteBranchInfo[], { projectId: string }>({
			extraOptions: { command: "git_remote_branches" },
			query: (args) => args,
			transformResponse: (data: string[]) => {
				return data
					.map((name) => name.substring(13))
					.sort((a, b) => a.localeCompare(b))
					.map((name) => ({ name }));
			},
		}),

		// ── Branch Listing ──────────────────────────────────────────
		listBranches: build.query<EntityState<BranchListing, string>, { projectId: string }>({
			extraOptions: { command: "list_branches" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.BranchListing)],
			transformResponse: (response: BranchListing[]) => {
				return listingAdapter.addMany(listingAdapter.getInitialState(), response);
			},
		}),
		branchListingDetails: build.query<
			BranchListingDetails,
			{ projectId: string; branchName: string }
		>({
			extraOptions: { command: "get_branch_listing_details" },
			query: ({ projectId, branchName }) => ({ projectId, branchNames: [branchName] }),
			transformResponse: (response: BranchListingDetails[]) => response.at(0)!,
			providesTags: [providesList(ReduxTag.BranchListing)],
		}),
	};
}

const listingAdapter = createEntityAdapter<BranchListing, string>({
	selectId: (listing) => listing.name,
});

export const listingSelectors = listingAdapter.getSelectors();
