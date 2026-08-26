import type { apiInvalidates } from "@gitbutler/but-sdk/cache-tags";

type DeclaredMutationKey = keyof typeof apiInvalidates;

type GlobalMutationKey = Extract<
	DeclaredMutationKey,
	| "addProject"
	| "deleteAllData"
	| "forgetBitbucketAccount"
	| "forgetGithubAccount"
	| "forgetGitlabAccount"
	| "resetAiConfiguration"
	| "storeBitbucketApiToken"
	| "storeGithubPat"
	| "storeGitlabPat"
	| "updateAiConfiguration"
>;

type ProjectMutationKey = Exclude<DeclaredMutationKey, GlobalMutationKey> | "commitAmend";

export type MutationKey =
	| readonly [projectId: string, ProjectMutationKey]
	| readonly [GlobalMutationKey];

declare module "@tanstack/react-query" {
	interface Register {
		mutationKey: MutationKey;
	}
}
