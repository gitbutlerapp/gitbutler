import { useOpenInProgram } from "#ui/api/mutations.ts";
import {
	guiSettingsQueryOptions,
	listEditorsQueryOptions,
	listProjectsQueryOptions,
	worktreesListQueryOptions,
} from "#ui/api/queries.ts";
import { changesFileHotkeys, revealInFolderLabel, toElectronAccelerator } from "#ui/hotkeys.ts";
import { type NativeMenuItem, nativeMenuItem } from "#ui/native-menu.ts";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";

/**
 * What a file offers wherever it is listed: open it, reveal it, copy its
 * path. None of these ask what the file belongs to, so surfaces with no notion of a
 * parent — edit mode, where every file belongs to the commit being edited —
 * can offer them too. Resolving the editor and the project's location still
 * takes queries, which every row calling this subscribes to.
 */
export const usePathMenuItems = ({
	projectId,
	path,
	worktree,
}: {
	projectId: string;
	path: string;
	/** The linked worktree the file lives in; the project's own checkout when unset. */
	worktree?: string;
}): Array<NativeMenuItem> => {
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const { data: worktreePath } = useQuery({
		...worktreesListQueryOptions(projectId),
		enabled: worktree !== undefined,
		select: (listing) =>
			[...listing.active, ...listing.archived].find((entry) => entry.name === worktree)?.path,
	});

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");
	// The checkout the path is relative to; unknown until the worktree listing answers.
	const basePath = worktree === undefined ? selectedProject.path : worktreePath;
	const absolutePath = () => window.lite.pathJoin(basePath ?? "", path);

	const editorMenuItem = useEditorMenuItem({
		projectId,
		// Linked worktree paths must be absolute; the backend resolves relative paths in the main checkout.
		path: worktree === undefined ? path : absolutePath,
		enabled: basePath !== undefined,
		accelerator: toElectronAccelerator(changesFileHotkeys.openInEditor.hotkey),
	});

	return [
		editorMenuItem,
		nativeMenuItem({
			label: revealInFolderLabel,
			enabled: basePath !== undefined,
			accelerator: toElectronAccelerator(changesFileHotkeys.revealInFolder.hotkey),
			onSelect: async () => window.lite.showItemInFolder(await absolutePath()),
		}),
		nativeMenuItem({
			label: "Copy Path",
			submenu: [
				nativeMenuItem({
					label: "Absolute Path",
					enabled: basePath !== undefined,
					onSelect: async () => window.lite.clipboardWriteText(await absolutePath()),
				}),
				nativeMenuItem({
					label: "Relative Path",
					onSelect: () => window.lite.clipboardWriteText(path),
				}),
			],
		}),
	];
};

export const useEditorMenuItem = ({
	projectId,
	path,
	lineNr = null,
	enabled = true,
	accelerator,
}: {
	projectId: string;
	path: string | (() => Promise<string>);
	lineNr?: number | null;
	enabled?: boolean;
	accelerator?: string;
}): NativeMenuItem => {
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});

	const { isPending, mutate: openInProgram } = useOpenInProgram();
	const openPath = async (programId: string) => {
		const target = typeof path === "string" ? path : await path();
		openInProgram({ projectId, programId, path: target, lineNr });
	};
	const canOpen = enabled && !isPending;
	return preferredEditor
		? nativeMenuItem({
				label: `Open in ${preferredEditor.name}`,
				enabled: canOpen,
				accelerator,
				onSelect: () => void openPath(preferredEditor.id),
			})
		: nativeMenuItem({
				label: "Open In Editor",
				submenu:
					editors?.map((editor) =>
						nativeMenuItem({
							label: editor.name,
							enabled: canOpen,
							onSelect: () => void openPath(editor.id),
						}),
					) ?? [],
			});
};
