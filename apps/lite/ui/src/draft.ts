import { queryOptions, useMutation } from "@tanstack/react-query";
import * as idb from "idb-keyval";

const draftCommitMessageKey = (projectId: string): string => `commit_message_draft:v1:${projectId}`;

export const draftCommitMessageQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [projectId, "commitMessageDraft"],
		queryFn: async () => (await idb.get<string>(draftCommitMessageKey(projectId))) ?? "",
	});

export const usePersistDraftCommitMessage = () =>
	useMutation({
		mutationFn: ({ projectId, message }: { projectId: string; message: string }) =>
			idb.set(draftCommitMessageKey(projectId), message),
		onSuccess: (_data, input, _res, ctx) =>
			ctx.client.setQueryData(
				draftCommitMessageQueryOptions(input.projectId).queryKey,
				input.message,
			),
	});
