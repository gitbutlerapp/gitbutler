import { invalidatesItem, providesItem, providesList, ReduxTag } from "$lib/state/tags";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type { DiffSpec } from "@gitbutler/but-sdk";
import type { GitConfigSettings } from "@gitbutler/but-sdk";

export function buildGitEndpoints(build: BackendEndpointBuilder) {
	return {
		// ── Git Config ──────────────────────────────────────────────
		gitGetGlobalConfig: build.query<unknown, { key: string }>({
			keepUnusedDataFor: 30,
			extraOptions: { command: "git_get_global_config" },
			query: (args) => args,
			transformResponse: (response: unknown) => {
				return response;
			},
			providesTags: (_result, _error, args) => providesItem(ReduxTag.GitConfigProperty, args.key),
		}),
		gitRemoveGlobalConfig: build.mutation<undefined, { key: string }>({
			extraOptions: { command: "git_remove_global_config" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.GitConfigProperty, args.key),
			],
		}),
		gitSetGlobalConfig: build.mutation<unknown, { key: string; value: unknown }>({
			extraOptions: { command: "git_set_global_config" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.GitConfigProperty, args.key),
			],
		}),
		gbConfig: build.query<GitConfigSettings, { projectId: string }>({
			extraOptions: { command: "get_gb_config" },
			query: (args) => args,
			providesTags: (_result, _error, args) =>
				providesItem(ReduxTag.GitButlerConfig, args.projectId),
		}),
		setGbConfig: build.mutation<void, { projectId: string; config: GitConfigSettings }>({
			extraOptions: { command: "set_gb_config" },
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.GitButlerConfig, args.projectId),
				invalidatesItem(ReduxTag.Project, args.projectId),
			],
		}),

		// ── Author Info ─────────────────────────────────────────────
		authorInfo: build.query<AuthorInfo, { projectId: string }>({
			extraOptions: { command: "get_author_info" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.AuthorInfo)],
		}),
		setAuthorInfo: build.mutation<void, { projectId: string; name: string; email: string }>({
			extraOptions: { command: "store_author_globally_if_unset" },
			query: (args) => args,
			invalidatesTags: [providesList(ReduxTag.AuthorInfo)],
		}),

		// ── Cherry Apply ────────────────────────────────────────────
		cherryApplyStatus: build.query<CherryApplyStatus, { projectId: string; subject: string }>({
			extraOptions: { command: "cherry_apply_status" },
			query: (args) => args,
		}),
		cherryApply: build.mutation<void, { projectId: string; subject: string; target: string }>({
			extraOptions: { command: "cherry_apply" },
			query: (args) => args,
		}),

		// ── Git Hooks ───────────────────────────────────────────────
		preCommitDiffspecs: build.mutation<HookStatus, { projectId: string; changes: DiffSpec[] }>({
			extraOptions: { command: "pre_commit_hook_diffspecs" },
			query: (args) => args,
		}),
		postCommit: build.mutation<HookStatus, { projectId: string }>({
			extraOptions: { command: "post_commit_hook" },
			query: (args) => args,
		}),
		messageHook: build.mutation<MessageHookStatus, { projectId: string; message: string }>({
			extraOptions: { command: "message_hook" },
			query: (args) => args,
		}),
	};
}

export type AuthorInfo = {
	name: string | null;
	email: string | null;
};

export type CherryApplyStatus =
	| {
			type: "causesWorkspaceConflict";
	  }
	| {
			type: "lockedToStack";
			subject: string;
	  }
	| {
			type: "applicableToAnyStack";
	  }
	| {
			type: "noStacks";
	  };

export type HookStatus =
	| {
			status: "success";
	  }
	| {
			status: "notconfigured";
	  }
	| {
			status: "failure";
			error: string;
	  };

export type MessageHookStatus =
	| {
			status: "success";
	  }
	| {
			status: "message";
			message: string;
	  }
	| {
			status: "notconfigured";
	  }
	| {
			status: "failure";
			error: string;
	  };
