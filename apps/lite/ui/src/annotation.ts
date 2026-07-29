import type {
	CommentArchiveParams,
	CommentCreateParams,
	CommentUpdateParams,
} from "#electron/ipc.ts";
import { commentsQueryOptions, type QueryKey } from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import type { FileParent } from "#ui/operands.ts";
import { Toast } from "@base-ui/react";
import type { DiffComment, DiffSide } from "@gitbutler/but-sdk";
import type { AnnotationSide } from "@pierre/diffs";
import { type QueryClient, useMutation } from "@tanstack/react-query";

/**
 * A backend comment anchored to a diff line, shaped for the diff view. Comments are read by
 * agents via `but comment list`, hence no draft/published status.
 */
export type LocalAnnotation = {
	id: string;
	lineNumber: number;
	side: AnnotationSide;
	body: string;
	updatedAtMs: number;
};

export type LocalAnnotationsByPath = Map<string, Array<LocalAnnotation>>;

/**
 * The backend anchor scope of a file parent: `null` for the uncommitted worktree diff, the
 * commit's change id for a commit diff, and `undefined` when comments are not supported
 * (branch diffs have no stable anchor scope).
 */
export const commentScopeChangeId = (fileParent: FileParent): string | null | undefined => {
	switch (fileParent._tag) {
		case "UncommittedChanges":
			return null;
		case "Commit":
			return fileParent.changeId;
		case "Branch":
			return undefined;
	}
};

export const annotationSideToDiffSide = (side: AnnotationSide): DiffSide =>
	side === "deletions" ? "old" : "new";

const diffSideToAnnotationSide = (side: DiffSide): AnnotationSide =>
	side === "old" ? "deletions" : "additions";

/** Group the comments belonging to the given scope by path, shaped for the diff view. */
export const annotationsByPathForScope = (
	comments: Array<DiffComment>,
	commitChangeId: string | null,
): LocalAnnotationsByPath => {
	const byPath: LocalAnnotationsByPath = new Map();
	for (const comment of comments) {
		if (comment.commitChangeId !== commitChangeId) continue;
		const annotations = byPath.get(comment.path) ?? [];
		annotations.push({
			id: comment.id,
			lineNumber: comment.lineNumber,
			side: diffSideToAnnotationSide(comment.side),
			body: comment.payload,
			updatedAtMs: comment.updatedAtMs,
		});
		byPath.set(comment.path, annotations);
	}
	return byPath;
};

const invalidateComments = (client: QueryClient, projectId: string) =>
	void client.invalidateQueries({ queryKey: ["comments" satisfies QueryKey, projectId] });

const useCommentErrorToast = (title: string) => {
	const toastManager = Toast.useToastManager();
	return (error: unknown) => {
		// oxlint-disable-next-line no-console
		console.error(error);
		toastManager.add({
			type: "error",
			title,
			description: errorMessageForToast(error),
			priority: "high",
		});
	};
};

export const useCommentCreate = () => {
	const onError = useCommentErrorToast("Failed to create comment");
	return useMutation({
		mutationFn: (params: CommentCreateParams) => window.lite.commentCreate(params),
		onSuccess: (comment, input, _result, ctx) =>
			ctx.client.setQueryData(commentsQueryOptions(input.projectId).queryKey, (comments) => [
				...(comments ?? []),
				comment,
			]),
		onError,
	});
};

export const useCommentUpdate = () => {
	const onError = useCommentErrorToast("Failed to save comment");
	return useMutation({
		mutationFn: (params: CommentUpdateParams) => window.lite.commentUpdate(params),
		onSuccess: (_data, input, _result, ctx) => invalidateComments(ctx.client, input.projectId),
		onError,
	});
};

export const useCommentArchive = () => {
	const onError = useCommentErrorToast("Failed to archive comment");
	return useMutation({
		mutationFn: (params: CommentArchiveParams) => window.lite.commentArchive(params),
		onSuccess: (_archived, input, _result, ctx) => invalidateComments(ctx.client, input.projectId),
		onError,
	});
};

const localAnnotationRevision = (fileParent: FileParent): string => {
	switch (fileParent._tag) {
		case "UncommittedChanges":
			return "working copy";
		case "Branch":
			return decodeBytes(fileParent.branchRef);
		case "Commit":
			return fileParent.commitId;
	}
};

export const feedbackPrompt = (
	allFeedback: Array<{
		annotation: LocalAnnotation;
		fileParent: FileParent;
		path: string;
	}>,
): string => `# Feedback

${allFeedback
	.map(
		(
			feedback,
			idx,
		) => `${idx + 1}. ${feedback.path}:${feedback.annotation.lineNumber} (${feedback.annotation.side}) in ${localAnnotationRevision(feedback.fileParent)}

${feedback.annotation.body}
`,
	)
	.join("\n")}`;
