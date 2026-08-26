import {
	useCommitDiscardChanges,
	useCommitUncommitChanges,
	useDiscardWorktreeChanges,
	useOpenInProgram,
} from "#ui/api/mutations.ts";
import { startAbsorb, startKeyboardTransfer } from "#ui/use-cursor.ts";
import {
	guiSettingsQueryOptions,
	listEditorsQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import {
	diffHotkeys,
	revealInFolderLabel,
	selectionOperationHotkeys,
	toElectronAccelerator,
} from "#ui/hotkeys.ts";
import { diffSpecHunkHeadersForLineSelection } from "#ui/hunk.ts";
import { type NativeMenuItem, nativeMenuItem, nativeMenuItemsFromGroups } from "#ui/native-menu.ts";
import { hunkAddress, type HunkAddress, type Address } from "#ui/addresses.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { useRevealInFolder } from "./useRevealInFolder.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { focusScope } from "#ui/focus-scopes.ts";
import { useAppStore } from "#ui/store.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { Match } from "effect";

type HunkMenuTarget = {
	change: TreeChange;
	hunk: HunkAddress;
	lineNumber: number;
	sources: Array<Extract<Address, { _tag: "Hunk" }>>;
	checkedProbe: Extract<Address, { _tag: "Hunk" }> | null;
	usesSelectedLines: boolean;
};

export const useHunkMenuItems = ({
	projectId,
}: {
	projectId: string;
}): ((target: HunkMenuTarget) => Array<NativeMenuItem>) => {
	const store = useAppStore();
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	const { isPending: isCommitUncommitChangesPending, mutate: commitUncommitChanges } =
		useCommitUncommitChanges();
	const { isPending: isCommitDiscardChangesPending, mutate: commitDiscardChanges } =
		useCommitDiscardChanges();
	const { isPending: isDiscardWorktreeChangesPending, mutate: discardWorktreeChanges } =
		useDiscardWorktreeChanges();
	const { isPending: isOpenInProgramPending, mutate: openInProgram } = useOpenInProgram();
	const revealInFolder = useRevealInFolder(projectId);

	return ({ sources, checkedProbe, usesSelectedLines, change, hunk, lineNumber }) => {
		const state = store.getState();
		const usesCheckedLines =
			checkedProbe !== null &&
			projectSlice.selectors.selectAddressChecked(state, projectId, checkedProbe);
		const cutSources = usesCheckedLines
			? projectSlice.selectors.selectCheckedAddresses(state, projectId)
			: sources;
		const canUseHunk = sources.every((source) => !source.isResultOfBinaryToTextConversion);
		const canCut = cutSources.every(
			(source) => source._tag !== "Hunk" || !source.isResultOfBinaryToTextConversion,
		);
		const cutHunk = () => {
			startKeyboardTransfer({ sources: cutSources, kind: "move" });
			focusScope("sidebar");
		};
		const discardDiffSpec = createDiffSpec(
			change,
			sources.flatMap((source) => diffSpecHunkHeadersForLineSelection(source, "discard")),
		);

		const menuItemGroups: Array<Array<NativeMenuItem>> = [
			[
				preferredEditor
					? nativeMenuItem({
							label: `Open in ${preferredEditor.name}`,
							enabled: !isOpenInProgramPending,
							accelerator: toElectronAccelerator(diffHotkeys.openInEditor.hotkey),
							onSelect: () =>
								openInProgram({
									projectId,
									programId: preferredEditor.id,
									path: change.path,
									lineNr: lineNumber,
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
												path: change.path,
												lineNr: lineNumber,
											}),
									}),
								) ?? [],
						}),
				nativeMenuItem({
					label: revealInFolderLabel,
					accelerator: toElectronAccelerator(diffHotkeys.revealInFolder.hotkey),
					onSelect: () => revealInFolder(change.path),
				}),
				nativeMenuItem({
					label: "Copy Path",
					submenu: [
						nativeMenuItem({
							label: "Absolute Path",
							onSelect: async () => {
								const absolutePath = await window.lite.pathJoin(selectedProject.path, change.path);
								await window.lite.clipboardWriteText(absolutePath);
							},
						}),
						nativeMenuItem({
							label: "Relative Path",
							onSelect: () => window.lite.clipboardWriteText(change.path),
						}),
					],
				}),
			],
			...(sources[0]?.parent.parent._tag !== "Branch"
				? [
						[
							nativeMenuItem({
								label: usesCheckedLines
									? "Cut Checked Lines"
									: usesSelectedLines
										? "Cut Selected Lines"
										: "Cut Hunk",
								enabled: canCut,
								onSelect: cutHunk,
								accelerator: toElectronAccelerator(selectionOperationHotkeys.cut.hotkey),
							}),
						] satisfies Array<NativeMenuItem>,
					]
				: []),
			...Match.value(sources[0]?.parent.parent).pipe(
				Match.withReturnType<Array<Array<NativeMenuItem>>>(),
				Match.when({ _tag: "Commit" }, ({ commitId }) => [
					[
						nativeMenuItem({
							label: usesSelectedLines ? "Uncommit Selected Lines" : "Uncommit Hunk",
							enabled: canUseHunk && !isCommitUncommitChangesPending,
							onSelect: () =>
								commitUncommitChanges({
									projectId,
									commitId,
									assignTo: null,
									changes: [discardDiffSpec],
									dryRun: false,
								}),
						}),
						nativeMenuItem({
							label: usesSelectedLines ? "Discard Selected Lines" : "Discard Hunk",
							enabled: canUseHunk && !isCommitDiscardChangesPending,
							onSelect: () =>
								commitDiscardChanges({
									projectId,
									commitId,
									changes: [discardDiffSpec],
									dryRun: false,
								}),
						}),
					],
				]),
				Match.when({ _tag: "UncommittedChanges" }, () => [
					[
						nativeMenuItem({
							label: "Absorb Hunk",
							enabled: !hunk.isResultOfBinaryToTextConversion,
							onSelect: () => {
								startAbsorb({
									sources: [hunkAddress(hunk)],
									sourceTarget: {
										type: "hunks",
										subject: {
											hunks: [{ pathBytes: change.pathBytes, hunkHeader: hunk.hunkHeader }],
										},
									},
								});

								focusScope("sidebar");
							},
							accelerator: toElectronAccelerator(diffHotkeys.absorb.hotkey),
						}),
						nativeMenuItem({
							label: usesSelectedLines ? "Discard Selected Lines" : "Discard Hunk",
							enabled: canUseHunk && !isDiscardWorktreeChangesPending,
							onSelect: () =>
								discardWorktreeChanges({
									projectId,
									worktreeChanges: [discardDiffSpec],
								}),
						}),
					],
				]),
				Match.orElse(() => []),
			),
		];

		return nativeMenuItemsFromGroups(menuItemGroups);
	};
};
