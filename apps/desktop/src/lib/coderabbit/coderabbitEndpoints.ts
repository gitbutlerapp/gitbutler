import { invalidatesList, providesList, ReduxTag } from "$lib/state/tags";
import type {
	CodeRabbitFinding,
	CodeRabbitFindingStatus,
	CodeRabbitReviewRequest,
	CodeRabbitReviewResult,
	CodeRabbitStatus,
} from "$lib/coderabbit/coderabbit";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";

export function buildCodeRabbitEndpoints(build: BackendEndpointBuilder) {
	return {
		coderabbitStatus: build.query<CodeRabbitStatus, { projectId: string }>({
			extraOptions: { command: "coderabbit_status" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.CodeRabbit)],
		}),
		coderabbitLogin: build.mutation<CodeRabbitStatus, { projectId: string }>({
			extraOptions: { command: "coderabbit_login" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.CodeRabbit)],
		}),
		coderabbitReview: build.mutation<
			CodeRabbitReviewResult,
			{ projectId: string; request: CodeRabbitReviewRequest }
		>({
			extraOptions: { command: "coderabbit_review" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.CodeRabbit)],
		}),
		coderabbitCancel: build.mutation<boolean, { projectId: string; reviewId: string }>({
			extraOptions: { command: "coderabbit_cancel" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.CodeRabbit)],
		}),
		coderabbitFindings: build.query<CodeRabbitFinding[], { projectId: string; reviewId?: string }>({
			extraOptions: { command: "coderabbit_findings" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.CodeRabbit)],
		}),
		coderabbitUpdateFinding: build.mutation<
			CodeRabbitFinding | undefined,
			{ projectId: string; update: { findingId: string; status: CodeRabbitFindingStatus } }
		>({
			extraOptions: { command: "coderabbit_update_finding" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.CodeRabbit)],
		}),
		coderabbitWriteDefaultConfig: build.mutation<boolean, { projectId: string }>({
			extraOptions: { command: "coderabbit_write_default_config" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.CodeRabbit)],
		}),
	};
}
