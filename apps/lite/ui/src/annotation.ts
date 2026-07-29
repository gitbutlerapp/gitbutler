import type {
	CommentArchiveParams,
	CommentCreateParams,
	CommentUpdateParams,
} from "#electron/ipc.ts";
import type { QueryKey } from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import type { FileParent } from "#ui/operands.ts";
import { Toast } from "@base-ui/react";
import type { DiffComment, DiffSide } from "@gitbutler/but-sdk";
import type { AnnotationSide } from "@pierre/diffs";
import { useMutation } from "@tanstack/react-query";

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

export const annotationSideToDiffSide = (side: AnnotationSide): DiffSide =>
	side === "deletions" ? "old" : "new";

const diffSideToAnnotationSide = (side: DiffSide): AnnotationSide =>
	side === "old" ? "deletions" : "additions";

/** Group the comments belonging to the given scope by path, shaped for the diff view. */
export const annotationsByPathForScope = (
	comments: Array<DiffComment>,
	fileParent: FileParent,
): LocalAnnotationsByPath => {
	const byPath: LocalAnnotationsByPath = new Map();

	// We don't currently support annotations on branches.
	if (fileParent._tag === "Branch") return byPath;

	const commitChangeId = fileParent._tag === "Commit" ? fileParent.changeId : null;
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

export const useCommentCreate = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (params: CommentCreateParams) => window.lite.commentCreate(params),
		onSettled: (_comment, _err, input, _result, ctx) =>
			ctx.client.invalidateQueries({ queryKey: ["comments" satisfies QueryKey, input.projectId] }),
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to create comment",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCommentUpdate = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (params: CommentUpdateParams) => window.lite.commentUpdate(params),
		onSettled: (_comment, _err, input, _result, ctx) =>
			ctx.client.invalidateQueries({ queryKey: ["comments" satisfies QueryKey, input.projectId] }),
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to update comment",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

export const useCommentArchive = () => {
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (params: CommentArchiveParams) => window.lite.commentArchive(params),
		onSettled: (_comment, _err, input, _result, ctx) =>
			ctx.client.invalidateQueries({ queryKey: ["comments" satisfies QueryKey, input.projectId] }),
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to archive comment",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
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
