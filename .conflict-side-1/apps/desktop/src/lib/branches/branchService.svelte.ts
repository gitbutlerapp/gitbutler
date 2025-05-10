import { providesList, ReduxTag } from '$lib/state/tags';
import type { BranchListing, BranchListingDetails } from '$lib/branches/branchListing';
import type { BackendApi, ClientState } from '$lib/state/clientState.svelte';

export class BranchService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(private readonly backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	list(projectId: string) {
		const result = $derived(this.api.endpoints.listBranches.useQuery({ projectId }));
		return result;
	}

	get(projectId: string, branchName: string) {
		const result = $derived(this.api.endpoints.branchDetails.useQuery({ projectId, branchName }));
		return result;
	}

	// TODO: Convert this to invalidation.
	async refresh(projectId: string): Promise<void> {
		await this.api.endpoints.listBranches.fetch({ projectId }, { forceRefetch: true });
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listBranches: build.query<BranchListing[], { projectId: string }>({
				query: ({ projectId }) => ({
					command: 'list_branches',
					params: { projectId }
				}),
				providesTags: [providesList(ReduxTag.BranchListing)]
			}),
			branchDetails: build.query<BranchListingDetails, { projectId: string; branchName: string }>({
				query: ({ projectId, branchName }) => ({
					command: 'get_branch_listing_details',
					params: { projectId, branchNames: [branchName] }
				}),
				transformResponse: (response: BranchListingDetails[]) => response.at(0)!,
				providesTags: [providesList(ReduxTag.BranchListing)]
			})
		})
	});
}
