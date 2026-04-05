import {
	invalidatesList,
	invalidatesType,
	providesList,
	providesType,
	ReduxTag,
} from "$lib/state/tags";
import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type { BaseBranch, ForgeProvider, RemoteBranchInfo } from "$lib/baseBranch/baseBranch";
import type { BranchListing, BranchListingDetails } from "$lib/branches/branchListing";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type {
	BaseBranchResolution,
	BaseBranchResolutionApproach,
	BranchStatusesResponse,
	IntegrationOutcome,
	Resolution,
} from "$lib/upstream/types";

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
		fetchFromRemotes: build.mutation<void, { projectId: string; action?: string }>({
			extraOptions: { command: "fetch_from_remotes" },
			query: ({ projectId, action }) => ({
				projectId,
				action: action ?? "auto",
			}),
			invalidatesTags: [
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
				invalidatesList(ReduxTag.UpstreamIntegrationStatus),
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
				invalidatesType(ReduxTag.BaseBranchData),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
			],
		}),
		switchBackToWorkspace: build.mutation<BaseBranch, { projectId: string }>({
			extraOptions: { command: "switch_back_to_workspace" },
			query: (args) => args,
			invalidatesTags: [
				invalidatesType(ReduxTag.ForgeProvider),
				invalidatesType(ReduxTag.BaseBranchData),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
			],
		}),
		pushBaseBranch: build.mutation<void, { projectId: string; withForce?: boolean }>({
			extraOptions: { command: "push_base_branch" },
			query: (args) => args,
			invalidatesTags: [invalidatesType(ReduxTag.BaseBranchData)],
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

		// ── Upstream Integration ─────────────────────────────────────
		upstreamIntegrationStatuses: build.query<
			BranchStatusesResponse,
			{ projectId: string; targetCommitOid?: string }
		>({
			extraOptions: { command: "upstream_integration_statuses" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.UpstreamIntegrationStatus)],
		}),
		integrateUpstream: build.mutation<
			IntegrationOutcome,
			{
				projectId: string;
				resolutions: Resolution[];
				baseBranchResolution?: BaseBranchResolution;
			}
		>({
			extraOptions: {
				command: "integrate_upstream",
				actionName: "Integrate Upstream",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.UpstreamIntegrationStatus),
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.BranchListing),
			],
		}),
		resolveUpstreamIntegration: build.mutation<
			string,
			{ projectId: string; resolutionApproach: { type: BaseBranchResolutionApproach } }
		>({
			extraOptions: {
				command: `resolve_upstream_integration`,
				actionName: "Resolve Integrate Upstream",
			},
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.UpstreamIntegrationStatus)],
		}),
	};
}

const listingAdapter = createEntityAdapter<BranchListing, string>({
	selectId: (listing) => listing.name,
});

export const listingSelectors = listingAdapter.getSelectors();
