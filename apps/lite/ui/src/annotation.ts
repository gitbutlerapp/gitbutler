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
