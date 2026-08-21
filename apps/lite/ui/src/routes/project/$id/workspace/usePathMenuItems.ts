import { useOpenInProgram } from "#ui/api/mutations.ts";
import {
	guiSettingsQueryOptions,
	listEditorsQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { changesFileHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import { type NativeMenuItem, nativeMenuItem } from "#ui/native-menu.ts";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";

/**
 * What a file offers wherever it is listed: open it, copy its path. These
 * need nothing but the path, so surfaces that have no notion of a file's
 * parent — edit mode, where every file belongs to the commit being edited —
 * can offer them too.
 */
export const usePathMenuItems = ({
	projectId,
	path,
}: {
	projectId: string;
	path: string;
}): Array<NativeMenuItem> => {
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	const { isPending: isOpenInProgramPending, mutate: openInProgram } = useOpenInProgram();

	return [
		preferredEditor
			? nativeMenuItem({
					label: `Open in ${preferredEditor.name}`,
					enabled: !isOpenInProgramPending,
					accelerator: toElectronAccelerator(changesFileHotkeys.openInEditor.hotkey),
					onSelect: () =>
						openInProgram({
							projectId,
							programId: preferredEditor.id,
							path,
							lineNr: null,
						}),
				})
			: nativeMenuItem({
					label: "Open In Editor",
					submenu:
						editors?.map((editor) =>
							nativeMenuItem({
								label: editor.name,
								enabled: !isOpenInProgramPending,
								onSelect: () =>
									openInProgram({
										projectId,
										programId: editor.id,
										path,
										lineNr: null,
									}),
							}),
						) ?? [],
				}),
		nativeMenuItem({
			label: "Copy Path",
			submenu: [
				nativeMenuItem({
					label: "Absolute Path",
					onSelect: async () => {
						const absolutePath = await window.lite.pathJoin(selectedProject.path, path);
						await window.lite.clipboardWriteText(absolutePath);
					},
				}),
				nativeMenuItem({
					label: "Relative Path",
					onSelect: () => window.lite.clipboardWriteText(path),
				}),
			],
		}),
	];
};
