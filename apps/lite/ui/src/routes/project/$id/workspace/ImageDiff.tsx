import { blobFileQueryOptions, workspaceFileQueryOptions } from "#ui/api/queries.ts";
import type { FileParent } from "#ui/addresses.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import type { FC } from "react";
import styles from "./ImageDiff.module.css";

type ImageSource =
	| { type: "workspace"; path: string; version: number }
	| { type: "blob"; path: string; blobId: string };

const imageSources = (
	change: TreeChange,
	fileParent: FileParent,
	version: number,
): { before: ImageSource | null; after: ImageSource | null } => {
	const isWorkspaceDiff = fileParent._tag === "UncommittedChanges";

	switch (change.status.type) {
		case "Addition":
			return {
				before: null,
				after: isWorkspaceDiff
					? { type: "workspace", path: change.path, version }
					: { type: "blob", path: change.path, blobId: change.status.subject.state.id },
			};
		case "Deletion":
			return {
				before: {
					type: "blob",
					path: change.path,
					blobId: change.status.subject.previousState.id,
				},
				after: null,
			};
		case "Modification":
			return {
				before: {
					type: "blob",
					path: change.path,
					blobId: change.status.subject.previousState.id,
				},
				after: isWorkspaceDiff
					? { type: "workspace", path: change.path, version }
					: { type: "blob", path: change.path, blobId: change.status.subject.state.id },
			};
		case "Rename":
			return {
				before: {
					type: "blob",
					path: change.status.subject.previousPath,
					blobId: change.status.subject.previousState.id,
				},
				after: isWorkspaceDiff
					? { type: "workspace", path: change.path, version }
					: { type: "blob", path: change.path, blobId: change.status.subject.state.id },
			};
	}
};

const useImageUrl = (projectId: string, source: ImageSource | null) => {
	const workspace = useQuery({
		...workspaceFileQueryOptions({
			projectId,
			relativePath: source?.path ?? "",
			version: source?.type === "workspace" ? source.version : 0,
		}),
		enabled: source?.type === "workspace",
	});
	const blob = useQuery({
		...blobFileQueryOptions({
			projectId,
			relativePath: source?.path ?? "",
			blobId: source?.type === "blob" ? source.blobId : "",
		}),
		enabled: source?.type === "blob",
	});

	const query = source?.type === "workspace" ? workspace : blob;
	const { content, mimeType } = query.data ?? {};
	const hasContent = content !== undefined && content !== null && content !== "";
	const isSvg = source?.path.toLowerCase().endsWith(".svg") === true;
	const url =
		hasContent && mimeType != null
			? `data:${mimeType};base64,${content}`
			: hasContent && isSvg
				? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(content)}`
				: null;
	return {
		url,
		isLoading: source !== null && query.isPending,
		isError: source !== null && (query.isError || (query.data !== undefined && url === null)),
	};
};

const ImagePanel: FC<{
	url: string | null;
	label: "Before" | "After";
	path: string;
	isLoading: boolean;
	isError: boolean;
}> = ({ url, label, path, isLoading, isError }) => (
	<div className={styles.panel}>
		<div className={styles.imageWrapper}>
			{isLoading ? (
				<span className="text-13">Loading image…</span>
			) : url !== null ? (
				<img src={url} alt={`${path} (${label})`} />
			) : (
				<span className="text-13">{isError ? "Could not load image" : "No image"}</span>
			)}
		</div>
		<div className="text-12">{label}</div>
	</div>
);

export const ImageDiff: FC<{
	projectId: string;
	change: TreeChange;
	fileParent: FileParent;
	version: number;
}> = ({ projectId, change, fileParent, version }) => {
	const sources = imageSources(change, fileParent, version);
	const before = useImageUrl(projectId, sources.before);
	const after = useImageUrl(projectId, sources.after);

	return (
		<div className={styles.container}>
			{sources.before && (
				<ImagePanel
					url={before.url}
					label="Before"
					path={sources.before.path}
					isLoading={before.isLoading}
					isError={before.isError}
				/>
			)}
			{sources.after && (
				<ImagePanel
					url={after.url}
					label="After"
					path={sources.after.path}
					isLoading={after.isLoading}
					isError={after.isError}
				/>
			)}
		</div>
	);
};
