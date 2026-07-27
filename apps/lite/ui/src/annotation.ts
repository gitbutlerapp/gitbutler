import { decodeBytes } from "#ui/api/bytes.ts";
import type { FileParent } from "#ui/operands.ts";
import type { AnnotationSide } from "@pierre/diffs";
import { queryOptions, useMutation } from "@tanstack/react-query";
import * as idb from "idb-keyval";

/** Local-only annotation intended for sending to an agent, hence no draft/published status. */
export type LocalAnnotation = {
	id: string;
	lineNumber: number;
	side: AnnotationSide;
	body: string;
};

export type LocalAnnotationsByPath = Map<string, Array<LocalAnnotation>>;

export const createLocalAnnotation = (
	lineNumber: number,
	side: AnnotationSide,
): LocalAnnotation => ({
	id: crypto.randomUUID(),
	lineNumber,
	side,
	body: "",
});

const localAnnotationsKey = ({
	projectId,
	fileParentKey,
}: {
	projectId: string;
	fileParentKey: string;
}) => `local_annotations:v1:${projectId}:${fileParentKey}`;

export const localAnnotationsQueryOptions = ({
	projectId,
	fileParentKey,
}: {
	projectId: string;
	fileParentKey: string;
}) =>
	queryOptions({
		queryKey: ["localAnnotations", projectId, fileParentKey],
		queryFn: async () =>
			(await idb.get<LocalAnnotationsByPath>(localAnnotationsKey({ projectId, fileParentKey }))) ??
			new Map(),
	});

export const usePersistLocalAnnotations = () =>
	useMutation({
		mutationFn: ({
			projectId,
			fileParentKey,
			annotations,
		}: {
			projectId: string;
			fileParentKey: string;
			annotations: LocalAnnotationsByPath;
		}) => {
			const key = localAnnotationsKey({ projectId, fileParentKey });
			return annotations.size === 0 ? idb.del(key) : idb.set(key, annotations);
		},
		onSuccess: (_data, input, _result, ctx) => {
			ctx.client.setQueryData(
				localAnnotationsQueryOptions({
					projectId: input.projectId,
					fileParentKey: input.fileParentKey,
				}).queryKey,
				input.annotations,
			);
		},
	});

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
