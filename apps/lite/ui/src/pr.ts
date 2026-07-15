import { mutationResult } from "#ui/api/mutation-result.ts";
import { liteApi, queryResult } from "#ui/api/queries.ts";
import { type AppDispatch, useAppDispatch } from "#ui/store.ts";
import * as idb from "idb-keyval";

type DraftPR = {
	title?: string;
	body?: string;
	isDraft?: boolean;
};

type DraftPRParams = { projectId: string; branchName: string };

// Branch name isn't stable identity. Ideally in the future this'd be written to Git metadata.
const draftPRKey = ({ projectId, branchName }: DraftPRParams): string =>
	`pr_draft:v1:${projectId}:${branchName}`;

const draftPRApi = liteApi.injectEndpoints({
	endpoints: (builder) => ({
		draftPR: builder.query<DraftPR | null, DraftPRParams>({
			queryFn: (params) =>
				queryResult(async () => (await idb.get<DraftPR>(draftPRKey(params))) ?? null),
			providesTags: (_data, _error, { projectId, branchName }) => [
				{ type: "DraftPR", id: `${projectId}:${branchName}` },
			],
		}),
		persistDraftPR: builder.mutation<
			void,
			DraftPRParams & {
				draft: DraftPR;
			}
		>({
			queryFn: ({ projectId, branchName, draft }) =>
				queryResult(() => idb.set(draftPRKey({ projectId, branchName }), draft)),
		}),
	}),
});

export const useDraftPRQuery = draftPRApi.useDraftPRQuery;

/** Move a draft PR, if any, from an old branch name to a new one following a rename. */
export const moveDraftPR = async ({
	dispatch,
	projectId,
	oldBranch,
	newBranch,
}: {
	dispatch: AppDispatch;
	projectId: string;
	oldBranch: string;
	newBranch: string;
}): Promise<void> => {
	const prevKey = draftPRKey({ projectId, branchName: oldBranch });
	const draft = await idb.get<DraftPR>(prevKey);
	if (!draft) return;

	await idb.set(draftPRKey({ projectId, branchName: newBranch }), draft);
	void dispatch(
		draftPRApi.util.upsertQueryData("draftPR", { projectId, branchName: newBranch }, draft),
	);

	await idb.del(prevKey);
	void dispatch(
		draftPRApi.util.upsertQueryData("draftPR", { projectId, branchName: oldBranch }, null),
	);
};

export const usePersistDraftPR = () => {
	const [trigger, state] = draftPRApi.usePersistDraftPRMutation();
	const dispatch = useAppDispatch();

	return mutationResult(trigger, state, {
		onSuccess: (_data, input) => {
			void dispatch(
				draftPRApi.util.upsertQueryData(
					"draftPR",
					{ projectId: input.projectId, branchName: input.branchName },
					input.draft,
				),
			);
		},
	});
};
