import {
	guiSettingsQueryOptions,
	listEditorsQueryOptions,
	worktreesListQueryOptions,
} from "#ui/api/queries.ts";
import { changesFileHotkeys, revealInFolderLabel, toElectronAccelerator } from "#ui/hotkeys.ts";
import { type NativeMenuItem, nativeMenuItem } from "#ui/native-menu.ts";
import { useQuery } from "@tanstack/react-query";
import {
	useAbsolutePath,
	useOpenPathInProgram,
	useRevealInFolder,
	worktreePathIn,
} from "./usePathActions.ts";

/**
 * What a file offers wherever it is listed: open it, reveal it, copy its
 * path. None of these ask what the file belongs to, so surfaces with no notion of a
 * parent — edit mode, where every file belongs to the commit being edited —
 * can offer them too. Resolving the editor still takes queries, which every row
 * calling this subscribes to.
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
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});
	// A linked worktree's location is unknown until its listing answers; until then
	// nothing that needs it is offered.
	const { data: isLocated = worktree === undefined } = useQuery({
		...worktreesListQueryOptions(projectId),
		enabled: worktree !== undefined,
		select: (listing) => worktree === undefined || worktreePathIn(listing, worktree) !== undefined,
	});

	const absolutePath = useAbsolutePath(projectId);
	const revealInFolder = useRevealInFolder(projectId);
	const { isPending: isOpenInProgramPending, openPathInProgram } = useOpenPathInProgram(projectId);
	const openPath = (programId: string) =>
		void openPathInProgram({ programId, path, lineNr: null, worktree });
	const canOpen = !isOpenInProgramPending && isLocated;

	return [
		preferredEditor
			? nativeMenuItem({
					label: `Open in ${preferredEditor.name}`,
					enabled: canOpen,
					accelerator: toElectronAccelerator(changesFileHotkeys.openInEditor.hotkey),
					onSelect: () => openPath(preferredEditor.id),
				})
			: nativeMenuItem({
					label: "Open In Editor",
					submenu:
						editors?.map((editor) =>
							nativeMenuItem({
								label: editor.name,
								enabled: canOpen,
								onSelect: () => openPath(editor.id),
							}),
						) ?? [],
				}),
		nativeMenuItem({
			label: revealInFolderLabel,
			enabled: isLocated,
			accelerator: toElectronAccelerator(changesFileHotkeys.revealInFolder.hotkey),
			onSelect: () => revealInFolder(path, worktree),
		}),
		nativeMenuItem({
			label: "Copy Path",
			submenu: [
				nativeMenuItem({
					label: "Absolute Path",
					enabled: isLocated,
					onSelect: async () => window.lite.clipboardWriteText(await absolutePath(path, worktree)),
				}),
				nativeMenuItem({
					label: "Relative Path",
					onSelect: () => window.lite.clipboardWriteText(path),
				}),
			],
		}),
	];
};
