<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileContextMenu from '$components/FileContextMenu.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import {
		conflictEntryHint,
		getConflictState,
		type ConflictEntryPresence
	} from '$lib/conflictEntryPresence';
	import { FILE_SERVICE } from '$lib/files/fileService';
	import { MODE_SERVICE, type EditModeMetadata } from '$lib/mode/modeService';
	import { vscodePath } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { createCommitSelection } from '$lib/selection/key';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { USER } from '$lib/user/user';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { getEditorUri, URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';

	import { Avatar, Badge, Button, FileListItem, InfoButton, Modal } from '@gitbutler/ui';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { SvelteSet } from 'svelte/reactivity';
	import { derived, fromStore, readable, toStore, type Readable } from 'svelte/store';
	import type { FileInfo } from '$lib/files/file';
	import type { TreeChange } from '$lib/hunks/change';
	import type { FileStatus } from '@gitbutler/ui/components/file/types';

	type Props = {
		projectId: string;
		editModeMetadata: EditModeMetadata;
	};

	const { projectId, editModeMetadata }: Props = $props();

	const projectService = inject(PROJECTS_SERVICE);
	const projectResult = $derived(projectService.getProject(projectId));

	const user = inject(USER);
	const stackService = inject(STACK_SERVICE);
	const modeService = inject(MODE_SERVICE);
	const userSettings = inject(SETTINGS);
	const fileService = inject(FILE_SERVICE);
	const urlService = inject(URL_SERVICE);

	const initialFiles = $derived(modeService.initialEditModeState({ projectId }));
	const uncommittedFiles = $derived(modeService.changesSinceInitialEditState({ projectId }));

	const [saveEdit, savingEdit] = modeService.saveEditAndReturnToWorkspaceMutation;
	const [abortEdit, abortingEdit] = modeService.abortEditAndReturnToWorkspaceMutation;

	function readFromWorkspace(
		filePath: string,
		projectId: string
	): Readable<{ data: FileInfo; isLarge: boolean } | undefined> {
		return readable(undefined as { data: FileInfo; isLarge: boolean } | undefined, (set) => {
			fileService.readFromWorkspace(filePath, projectId).then(set);
		});
	}

	let commitResult = $derived(stackService.commitDetails(projectId, editModeMetadata.commitOid));

	let filesList = $state<HTMLDivElement | undefined>(undefined);
	let contextMenu = $state<ReturnType<typeof FileContextMenu> | undefined>(undefined);
	let confirmSaveModal = $state<ReturnType<typeof Modal> | undefined>(undefined);

	interface FileEntry {
		path: string;
		status?: FileStatus;
		conflicted: boolean;
		conflictHint?: string;
	}

	const initialFileMap = $derived(
		new Map<string, { file: TreeChange; conflictEntryPresence?: ConflictEntryPresence }>(
			initialFiles.current?.data?.map(([f, c]) => [
				f.path,
				{ file: f, conflictEntryPresence: c }
			]) || []
		)
	);

	const files = $derived.by(() => {
		if (!initialFiles.current.data || !uncommittedFiles.current.data) return [];

		const outputMap = new Map<string, FileEntry>();

		initialFiles.current.data.forEach(([initialFile, conflictEntryPresence]) => {
			outputMap.set(initialFile.path, {
				path: initialFile.path,
				conflicted: !!conflictEntryPresence,
				conflictHint: conflictEntryPresence ? conflictEntryHint(conflictEntryPresence) : undefined
			});
		});

		uncommittedFiles.current.data.forEach((uncommitedFile) => {
			const outputFile = outputMap.get(uncommitedFile.path)!;
			if (outputFile) {
				// We don't want to set a status if the file is
				// conflicted because it will _always_ show up as
				// modified
				if (!outputFile.conflicted) {
					outputFile.status = computeChangeStatus(uncommitedFile);
				}
				return;
			}

			outputMap.set(uncommitedFile.path, {
				path: uncommitedFile.path,
				conflicted: false,
				status: computeChangeStatus(uncommitedFile)
			});
		});

		const orderedOutput = Array.from(outputMap.values());
		orderedOutput.sort((a, b) => {
			// Float conflicted files to the top
			if (a.conflicted && !b.conflicted) {
				return -1;
			} else if (!a.conflicted && b.conflicted) {
				return 1;
			}

			return a.path.localeCompare(b.path);
		});

		return orderedOutput;
	});

	// Create a mapping from file paths to TreeChange objects for context menu
	const filePathToTreeChange = $derived(
		new Map<string, TreeChange>([
			...(initialFiles.current?.data?.map(([f]) => [f.path, f] as [string, TreeChange]) || []),
			...(uncommittedFiles.current?.data?.map((f) => [f.path, f] as [string, TreeChange]) || [])
		])
	);

	function getTreeChangeForFile(file: FileEntry): TreeChange | undefined {
		return filePathToTreeChange.get(file.path);
	}

	const conflictedFiles = $derived(files.filter((file) => file.conflicted));

	let manuallyResolvedFiles = new SvelteSet<string>();
	const filesWithConflictedStatues = $derived(
		conflictedFiles.map((f) => [f, isConflicted(f)] as [FileEntry, Readable<boolean>])
	);
	const stillConflictedFiles = $derived(
		filesWithConflictedStatues.filter(([_, status]) => fromStore(status).current).map(([f]) => f)
	);

	function isConflicted(fileEntry: FileEntry): Readable<boolean> {
		const file = readFromWorkspace(fileEntry.path, projectId);
		const conflictState = derived(file, (file) => {
			if (!isDefined(file?.data.content)) return 'unknown';
			const { conflictEntryPresence } = initialFileMap.get(fileEntry.path) || {};
			if (!conflictEntryPresence) return 'unknown';
			return getConflictState(conflictEntryPresence, file.data.content);
		});

		const manuallyResolved = toStore(() => manuallyResolvedFiles.has(fileEntry.path));

		return derived([conflictState, manuallyResolved], ([conflictState, manuallyResolved]) => {
			return fileEntry.conflicted && conflictState === 'conflicted' && !manuallyResolved;
		});
	}

	async function abort() {
		await abortEdit({ projectId });
	}

	async function save() {
		await saveEdit({ projectId });
	}

	async function handleSave() {
		if (stillConflictedFiles.length > 0) {
			confirmSaveModal?.show();
			return;
		}

		await save();
	}

	async function openAllConflictedFiles(projectPath: string) {
		for (const file of conflictedFiles) {
			const path = getEditorUri({
				schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
				path: [vscodePath(projectPath), file.path]
			});
			urlService.openExternalUrl(path);
		}
	}

	let isCommitListScrolled = $state(false);

	const loading = $derived(savingEdit.current.isLoading || abortingEdit.current.isLoading);
</script>

<div class="editmode-wrapper">
	<ReduxResult {projectId} result={projectResult.current}>
		{#snippet children(project)}
			<div class="editmode__container">
				<h2 class="editmode__title text-18 text-body text-bold">
					You are editing commit <span class="code-string">
						{editModeMetadata.commitOid.slice(0, 7)}
					</span>
					<InfoButton title="Edit Mode">
						Edit Mode lets you modify an existing commit in isolation or resolve conflicts. Any
						changes made, including new files, will be added to the selected commit.
					</InfoButton>
				</h2>

				<div class="commit-group">
					<div class="card commit-card">
						<ReduxResult {projectId} result={commitResult.current}>
							{#snippet children(commit)}
								{@const authorImgUrl = commit
									? commit.author.email?.toLowerCase() === $user?.email?.toLowerCase()
										? $user?.picture
										: commit.author.gravatarUrl
									: undefined}
								{@const title = splitMessage(commit.message).title}
								<h3 class="text-13 text-semibold text-body commit-card__title">
									{title || 'Undefined commit'}
								</h3>

								{#if commit}
									<div class="text-11 commit-card__details">
										{#if authorImgUrl && commit.author.email}
											<Avatar srcUrl={authorImgUrl} tooltip={commit.author.email} />
											<span class="commit-card__divider">•</span>
										{/if}
										<span class="">{editModeMetadata.commitOid.slice(0, 7)}</span>
										<span class="commit-card__divider">•</span>
										<span class="">{commit.author.name}</span>
									</div>
								{/if}

								<div class="commit-card__type-indicator"></div>
							{/snippet}
						</ReduxResult>
					</div>

					<div bind:this={filesList} class="card files">
						<div class="header" class:show-border={isCommitListScrolled}>
							<h3 class="text-13 text-semibold">Commit files</h3>
							<Badge>{files.length}</Badge>
						</div>
						<ScrollableContainer
							onscrollTop={(visible) => {
								isCommitListScrolled = !visible;
							}}
						>
							{#each files as file (file.path)}
								{@const conflictedStore = isConflicted(file)}
								{@const conflicted = fromStore(conflictedStore).current}
								<div class="file">
									<FileListItem
										filePath={file.path}
										fileStatus={file.status}
										{conflicted}
										onresolveclick={file.conflicted
											? () => manuallyResolvedFiles.add(file.path)
											: undefined}
										conflictHint={file.conflictHint}
										onclick={(e) => {
											const treeChange = getTreeChangeForFile(file);
											if (treeChange) {
												contextMenu?.open(e, { changes: [treeChange] });
											}
										}}
										oncontextmenu={(e) => {
											const treeChange = getTreeChangeForFile(file);
											if (treeChange) {
												contextMenu?.open(e, { changes: [treeChange] });
											}
										}}
									/>
								</div>
							{/each}
						</ScrollableContainer>
					</div>
				</div>

				<p class="text-12 text-body editmode__helptext">
					Please don't make any commits while in edit mode.
					<br />
					To exit edit mode, use the provided actions.
				</p>

				<div class="editmode__actions">
					<Button kind="outline" onclick={abort} disabled={loading} {loading}>Cancel</Button>
					{#if conflictedFiles.length > 0}
						<Button
							style="neutral"
							onclick={() => openAllConflictedFiles(project.path)}
							tooltip={conflictedFiles.length === 1
								? 'Open the conflicted file in your editor'
								: 'Open all files with conflicts in your editor'}
						>
							Open conflicted files
						</Button>
					{/if}
					<Button style="pop" icon="tick-small" onclick={handleSave} disabled={loading} {loading}>
						Save and exit
					</Button>
				</div>
			</div>

			<FileContextMenu
				bind:this={contextMenu}
				{projectId}
				trigger={filesList}
				stackId={undefined}
				selectionId={createCommitSelection({ commitId: editModeMetadata.commitOid })}
				editMode={true}
			/>
		{/snippet}
	</ReduxResult>
</div>

<Modal
	bind:this={confirmSaveModal}
	title="Save and exit"
	type="warning"
	width="small"
	onSubmit={async (close) => {
		await save();
		close();
	}}
>
	<p class="text-13 text-body helper-text">
		There are still some files that look to be conflicted. Are you sure that you want to save and
		exit?
	</p>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="error" type="submit">Save and exit</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.editmode-wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.editmode__container {
		--side-padding: 40px;
		display: flex;
		flex-direction: column;
		justify-content: center;
		width: 100%;
		max-width: calc(520px + 2 * var(--side-padding));
		margin: 0 auto;
		padding: 40px var(--side-padding) 24px;
		overflow: hidden;
	}

	.editmode__title {
		margin-bottom: 12px;
		color: var(--clr-text-1);
	}

	.editmode__actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		padding-bottom: 24px;
		gap: 8px;
	}

	.files {
		flex: 1;
		margin-bottom: 12px;
		padding-bottom: 8px;
		overflow: hidden;

		& .header {
			display: flex;
			align-items: center;
			padding-top: 16px;
			padding-bottom: 8px;
			padding-left: 16px;
			gap: 4px;

			&.show-border {
				border-bottom: 1px solid var(--clr-border-2);
			}
		}

		& .file {
			border-bottom: 1px solid var(--clr-border-3);
			&:last-child {
				border-bottom: none;
			}
		}
	}

	.code-string {
		margin-right: 2px;
	}

	/* COMMIT */
	.commit-group {
		display: flex;
		position: relative;
		flex-direction: column;
		overflow: hidden;
		gap: 4px;
	}

	/* COMMIT CARD */
	.commit-card {
		position: relative;
		padding: 14px 14px 14px 16px;
		overflow: hidden;
		gap: 8px;
	}

	.commit-card__title {
		color: var(--clr-text-1);
	}

	.commit-card__details {
		display: flex;
		gap: 4px;
		color: var(--clr-text-2);
	}

	.commit-card__type-indicator {
		position: absolute;
		top: 0;
		left: 0;
		width: 4px;
		height: 100%;
		background-color: var(--clr-commit-local);
	}

	.editmode__helptext {
		margin-bottom: 16px;
		color: var(--clr-text-3);
	}
</style>
