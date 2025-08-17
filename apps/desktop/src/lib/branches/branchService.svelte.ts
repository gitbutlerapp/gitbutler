import { invalidatesList, providesList, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/shared/context';
import type { BranchListing, BranchListingDetails } from '$lib/branches/branchListing';
import type { BackendApi, ClientState } from '$lib/state/clientState.svelte';

export const BRANCH_SERVICE = new InjectionToken<BranchService>('BranchService');

export class BranchService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(private readonly backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	list(projectId: string) {
		return this.api.endpoints.listBranches.useQuery({ projectId });
	}

	get(projectId: string, branchName: string) {
		return this.api.endpoints.branchDetails.useQuery({ projectId, branchName });
	}

	async refresh(): Promise<void> {
		this.api.util.invalidateTags([invalidatesList(ReduxTag.BranchListing)]);
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listBranches: build.query<BranchListing[], { projectId: string }>({
				extraOptions: { command: 'list_branches' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.BranchListing)]
			}),
			branchDetails: build.query<BranchListingDetails, { projectId: string; branchName: string }>({
				extraOptions: { command: 'get_branch_listing_details' },
				query: ({ projectId, branchName }) => ({ projectId, branchNames: [branchName] }),
				transformResponse: (response: BranchListingDetails[]) => response.at(0)!,
				providesTags: [providesList(ReduxTag.BranchListing)]
			})
		})
	});
}
