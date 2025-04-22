<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileContextMenu from '$components/FileContextMenu.svelte';
	import { Commit } from '$lib/commits/commit';
	import { CommitService } from '$lib/commits/commitService.svelte';
	import {
		conflictEntryHint,
		getConflictState,
		getInitialFileStatus,
		type ConflictEntryPresence,
		type ConflictState
	} from '$lib/conflictEntryPresence';
	import { type RemoteFile } from '$lib/files/file';
	import { UncommitedFilesWatcher } from '$lib/files/watcher';
	import { ModeService, type EditModeMetadata } from '$lib/mode/modeService';
	import { Project } from '$lib/project/project';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { UserService } from '$lib/user/userService';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import InfoButton from '@gitbutler/ui/InfoButton.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import type { FileStatus } from '@gitbutler/ui/file/types';
	import type { Writable } from 'svelte/store';

	interface Props {
		editModeMetadata: EditModeMetadata;
	}

	const { editModeMetadata }: Props = $props();

	const project = getContext(Project);
	const remoteCommitService = getContext(CommitService);
	const uncommitedFileWatcher = getContext(UncommitedFilesWatcher);
	const modeService = getContext(ModeService);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	const uncommitedFiles = uncommitedFileWatcher.uncommitedFiles;

	const userService = getContext(UserService);
	const user = userService.user;

	let modeServiceAborting = $state<'inert' | 'loading' | 'completed'>('inert');
	let modeServiceSaving = $state<'inert' | 'loading' | 'completed'>('inert');

	let initialFiles = $state<[RemoteFile, ConflictEntryPresence | undefined][]>([]);
	let commit = $state<Commit>();

	async function getCommitData() {
		commit = await remoteCommitService.find(project.id, editModeMetadata.commitOid);
	}

	$effect(() => {
		getCommitData();
	});

	const authorImgUrl = $derived.by(() => {
		if (commit) {
			return commit.author.email?.toLowerCase() === $user?.email?.toLowerCase()
				? $user?.picture
				: commit.author.gravatarUrl;
		}
		return undefined;
	});

	let filesList = $state<HTMLDivElement | undefined>(undefined);
	let contextMenu = $state<ReturnType<typeof FileContextMenu> | undefined>(undefined);
	let confirmSaveModal = $state<ReturnType<typeof Modal> | undefined>(undefined);

	$effect(() => {
		modeService.getInitialIndexState().then((files) => {
			initialFiles = files;
		});
	});

	interface FileEntry {
		conflicted: boolean;
		name: string;
		path: string;
		status?: FileStatus;
		conflictHint?: string;
		conflictState?: ConflictState;
		conflictEntryPresence?: ConflictEntryPresence;
	}

	const initialFileMap = $derived(
		new Map<string, RemoteFile>(initialFiles.map(([file]) => [file.path, file]))
	);

	const uncommitedFileMap = $derived(
		new Map<string, RemoteFile>($uncommitedFiles.map(([file]) => [file.path, file]))
	);

	const files = $derived.by(() => {
		const outputMap = new Map<string, FileEntry>();

		// Create output
		{
			initialFiles.forEach(([initialFile, conflictEntryPresence]) => {
				const conflictState =
					conflictEntryPresence && getConflictState(initialFile, conflictEntryPresence);

				const uncommitedFileChange = uncommitedFileMap.get(initialFile.path);

				outputMap.set(initialFile.path, {
					name: initialFile.filename,
					path: initialFile.path,
					conflicted: !!conflictEntryPresence,
					conflictHint: conflictEntryPresence
						? conflictEntryHint(conflictEntryPresence)
						: undefined,
					status: getInitialFileStatus(uncommitedFileChange, conflictEntryPresence),
					conflictState,
					conflictEntryPresence
				});
			});

			$uncommitedFiles.forEach(([uncommitedFile]) => {
				const existingFile = initialFileMap.get(uncommitedFile.path);
				determineOutput: {
					if (existingFile) {
						const fileChanged = existingFile.hunks.some(
							(hunk) => !uncommitedFile.hunks.map((hunk) => hunk.diff).includes(hunk.diff)
						);

						if (fileChanged) {
							// All initial entries should have been added to the map,
							// so we can safely assert that it will be present
							const outputFile = outputMap.get(uncommitedFile.path)!;
							if (outputFile.conflicted && outputFile.conflictEntryPresence) {
								outputFile.conflictState = getConflictState(
									uncommitedFile,
									outputFile.conflictEntryPresence
								);
							}

							if (!outputFile.conflicted) {
								outputFile.status = 'M';
							}
						}
						break determineOutput;
					}

					outputMap.set(uncommitedFile.path, {
						name: uncommitedFile.filename,
						path: uncommitedFile.path,
						conflicted: false,
						status: computeFileStatus(uncommitedFile)
					});
				}
			});
		}

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

	const conflictedFiles = $derived(files.filter((file) => file.conflicted));
	let manuallyResolvedFiles = new SvelteSet<string>();
	const stillConflictedFiles = $derived(
		conflictedFiles.filter(
			(file) => !manuallyResolvedFiles.has(file.path) && file.conflictState !== 'resolved'
		)
	);

	function isConflicted(file: FileEntry): boolean {
		return (
			file.conflicted && file.conflictState !== 'resolved' && !manuallyResolvedFiles.has(file.path)
		);
	}

	async function abort() {
		modeServiceAborting = 'loading';

		try {
			await modeService.abortEditAndReturnToWorkspace();
			modeServiceAborting = 'completed';
		} finally {
			modeServiceAborting = 'inert';
		}
	}

	async function save() {
		modeServiceSaving = 'loading';

		try {
			await modeService.saveEditAndReturnToWorkspace();
			modeServiceSaving = 'completed';
		} finally {
			modeServiceAborting = 'inert';
		}
	}

	async function handleSave() {
		if (stillConflictedFiles.length > 0) {
			confirmSaveModal?.show();
			return;
		}

		await save();
	}

	async function openAllConflictedFiles() {
		for (const file of conflictedFiles) {
			const path = getEditorUri({
				schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
				path: [project.vscodePath, file.path]
			});
			openExternalUrl(path);
		}
	}

	let isCommitListScrolled = $state(false);

	const loading = $derived(modeServiceSaving === 'loading' || modeServiceAborting === 'loading');
</script>

<div class="editmode__container">
	<h2 class="editmode__title text-18 text-body text-bold">
		You are editing commit <span class="code-string">
			{editModeMetadata.commitOid.slice(0, 7)}
		</span>
		<InfoButton title="Edit Mode">
			Edit Mode lets you modify an existing commit in isolation or resolve conflicts. Any changes
			made, including new files, will be added to the selected commit.
		</InfoButton>
	</h2>

	<div class="commit-group">
		<div class="card commit-card">
			<h3 class="text-13 text-semibold text-body commit-card__title">
				{commit?.descriptionTitle || 'Undefined commit'}
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
		</div>

		<div bind:this={filesList} class="card files">
			<div class="header" class:show-border={isCommitListScrolled}>
				<h3 class="text-13 text-semibold">Commit files</h3>
				<Badge>{files.length}</Badge>
			</div>
			<ScrollableContainer
				onscroll={(e) => {
					if (e.target instanceof HTMLElement) {
						isCommitListScrolled = e.target.scrollTop > 0;
					}
				}}
			>
				{#each files as file (file.path)}
					<div class="file">
						<FileListItem
							filePath={file.path}
							fileStatus={file.status}
							conflicted={isConflicted(file)}
							onresolveclick={file.conflicted
								? () => manuallyResolvedFiles.add(file.path)
								: undefined}
							conflictHint={file.conflictHint}
							onclick={(e) => {
								contextMenu?.open(e, { files: [file] });
							}}
							oncontextmenu={(e) => {
								contextMenu?.open(e, { files: [file] });
							}}
						/>
					</div>
				{/each}
			</ScrollableContainer>
		</div>
	</div>

	<FileContextMenu
		bind:this={contextMenu}
		trigger={filesList}
		isUnapplied={false}
		stackId={undefined}
		projectId={project.id}
	/>

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
				onclick={openAllConflictedFiles}
				icon="open-link"
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
	{#snippet children()}
		<p class="text-13 text-body helper-text">
			There are still some files that look to be conflicted. Are you sure that you want to save and
			exit?
		</p>
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="error" type="submit">Save and exit</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.editmode__container {
		--side-padding: 40px;
		display: flex;
		flex-direction: column;
		justify-content: center;
		margin: 0 auto;
		padding: 40px var(--side-padding) 24px;
		overflow: hidden;
		width: 100%;
		max-width: calc(520px + 2 * var(--side-padding));
	}

	.editmode__title {
		color: var(--clr-text-1);
		margin-bottom: 12px;
	}

	.editmode__actions {
		display: flex;
		gap: 8px;
		padding-bottom: 24px;
		flex-wrap: wrap;
		justify-content: flex-end;
	}

	.files {
		flex: 1;
		margin-bottom: 12px;
		overflow: hidden;
		padding-bottom: 8px;

		& .header {
			display: flex;
			align-items: center;
			gap: 4px;
			padding-left: 16px;
			padding-top: 16px;
			padding-bottom: 8px;

			&.show-border {
				border-bottom: 1px solid var(--clr-border-3);
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
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 4px;
		overflow: hidden;
	}

	/* COMMIT CARD */
	.commit-card {
		position: relative;
		padding: 14px 14px 14px 16px;
		gap: 8px;
		overflow: hidden;
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
		color: var(--clr-text-3);
		margin-bottom: 16px;
	}
</style>
