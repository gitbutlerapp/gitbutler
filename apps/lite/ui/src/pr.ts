import type { ForgeInfo } from "@gitbutler/but-sdk";
import { type QueryClient, queryOptions, useMutation } from "@tanstack/react-query";
import * as idb from "idb-keyval";

export const prForgeUrl = (prNo: number, forge: ForgeInfo): string =>
	`${forge.baseUrl}${forge.prUrlPath}${prNo}`;

type DraftPR = {
	title?: string;
	body?: string;
	isDraft?: boolean;
};

// Branch name isn't stable identity. Ideally in the future this'd be written to Git metadata.
const draftPRKey = ({ projectId, branchName }: { projectId: string; branchName: string }): string =>
	`pr_draft:v1:${projectId}:${branchName}`;

/** Move a draft PR, if any, from an old branch name to a new one following a rename. */
export const moveDraftPR = async ({
	queryClient,
	projectId,
	oldBranch,
	newBranch,
}: {
	queryClient: QueryClient;
	projectId: string;
	oldBranch: string;
	newBranch: string;
}): Promise<void> => {
	const prevKey = draftPRKey({ projectId, branchName: oldBranch });
	const draft = await idb.get<DraftPR>(prevKey);
	if (!draft) return;

	const newKey = draftPRKey({ projectId, branchName: newBranch });
	await idb.set(newKey, draft);
	queryClient.setQueryData(
		draftPRQueryOptions({ projectId, branchName: newBranch }).queryKey,
		draft,
	);

	await idb.del(prevKey);
	queryClient.removeQueries({
		queryKey: draftPRQueryOptions({ projectId, branchName: oldBranch }).queryKey,
	});
};

export const draftPRQueryOptions = ({
	projectId,
	branchName,
}: {
	projectId: string;
	branchName: string;
}) =>
	queryOptions({
		queryKey: ["prDraft", projectId, branchName],
		queryFn: async () => (await idb.get<DraftPR>(draftPRKey({ projectId, branchName }))) ?? null,
	});

export const usePersistDraftPR = () =>
	useMutation({
		mutationFn: ({
			projectId,
			branchName,
			draft,
		}: {
			projectId: string;
			branchName: string;
			draft: DraftPR;
		}) => idb.set(draftPRKey({ projectId, branchName }), draft),
		onSuccess: (_data, input, _res, ctx) =>
			ctx.client.setQueryData(
				draftPRQueryOptions({ projectId: input.projectId, branchName: input.branchName }).queryKey,
				input.draft,
			),
	});
