import { Commit } from './commit';
import { ReduxTag } from '$lib/state/tags';
import { plainToInstance } from 'class-transformer';
import type { HunkHeader } from '$lib/hunks/hunk';
import type { ClientState } from '$lib/state/clientState.svelte';

type CreateCommitRequest = {
	message: string;
	parentId: string;
	worktreeChanges: {
		previousPathBytes?: number[];
		pathBytes: number[];
		hunkHeaders: HunkHeader[];
	}[];
};

export class CommitService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.backendApi);
	}

	find(projectId: string, commitOid: string) {
		const result = $derived(this.api.endpoints.find.useQuery({ projectId, commitOid }));
		return result;
	}

	// eslint-disable-next-line @typescript-eslint/promise-function-async
	createCommit(projectId: string, request: CreateCommitRequest) {
		throw new Error('Not yet implemented');
		const result = $derived(this.api.endpoints.createCommit.useMutation({ projectId, ...request }));
		return result;
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			find: build.query<Commit, { projectId: string; commitOid: string }>({
				query: ({ projectId, commitOid }) => ({
					command: 'find_commit',
					params: { projectId, commitOid }
				}),
				transformResponse: (response: unknown) => plainToInstance(Commit, response),
				providesTags: [ReduxTag.Commit]
			}),
			createCommit: build.mutation<Commit, { projectId: string } & CreateCommitRequest>({
				query: ({ projectId, ...commitData }) => ({
					command: 'create_commit_from_worktree_changes',
					params: { projectId, ...commitData }
				}),
				invalidatesTags: [ReduxTag.Commits]
			})
		})
	});
}
