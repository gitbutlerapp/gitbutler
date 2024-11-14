<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import { CommitService } from '$lib/commits/service';
	import { conflictEntryHint, type ConflictEntryPresence } from '$lib/conflictEntryPresence';
	import FileContextMenu from '$lib/file/FileContextMenu.svelte';
	import { ModeService, type EditModeMetadata } from '$lib/modes/service';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { UserService } from '$lib/stores/user';
	import { UncommitedFilesWatcher } from '$lib/uncommitedFiles/watcher';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { Commit, type RemoteFile } from '$lib/vbranches/types';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import InfoButton from '@gitbutler/ui/InfoButton.svelte';
	import Avatar from '@gitbutler/ui/avatar/Avatar.svelte';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
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
	let commit = $state<Commit | undefined>(undefined);
	let authorImgUrl = $state<string | undefined>(undefined);

	let filesList = $state<HTMLDivElement | undefined>(undefined);
	let contextMenu = $state<ReturnType<typeof FileContextMenu> | undefined>(undefined);

	$effect(() => {
		modeService.getInitialIndexState().then((files) => {
			initialFiles = files;
		});
	});

	$effect(() => {
		remoteCommitService.find(editModeMetadata.commitOid).then((maybeCommit) => {
			commit = maybeCommit;
		});
	});

	$effect(() => {
		if (commit) {
			authorImgUrl =
				commit.author.email?.toLowerCase() === $user?.email?.toLowerCase()
					? $user?.picture
					: commit.author.gravatarUrl;
		}
	});

	interface FileEntry {
		name: string;
		path: string;
		conflicted: boolean;
		conflictHint?: string;
		status?: FileStatus;
	}

	const files = $derived.by(() => {
		const initialFileMap = new Map<string, RemoteFile>();
		const uncommitedFileMap = new Map<string, RemoteFile>();
		const outputMap = new Map<string, FileEntry>();

		// Build maps of files
		{
			initialFiles.forEach(([initialFile]) => {
				initialFileMap.set(initialFile.path, initialFile);
			});

			$uncommitedFiles.forEach(([uncommitedFile]) => {
				uncommitedFileMap.set(uncommitedFile.path, uncommitedFile);
			});
		}

		// Create output
		{
			initialFiles.forEach(([initialFile, conflictEntryPresence]) => {
				const isDeleted = uncommitedFileMap.has(initialFile.path);

				if (conflictEntryPresence) {
					console.log(initialFile.path, conflictEntryPresence);
				}

				outputMap.set(initialFile.path, {
					name: initialFile.filename,
					path: initialFile.path,
					conflicted: !!conflictEntryPresence,
					conflictHint: conflictEntryPresence
						? conflictEntryHint(conflictEntryPresence)
						: undefined,
					status: isDeleted || !!conflictEntryPresence ? undefined : 'D'
				});
			});

			$uncommitedFiles.forEach(([uncommitedFile]) => {
				const existingFile = initialFileMap.get(uncommitedFile.path);

				if (existingFile) {
					const fileChanged = existingFile.hunks.some(
						(hunk) => !uncommitedFile.hunks.map((hunk) => hunk.diff).includes(hunk.diff)
					);

					if (fileChanged) {
						// All initial entries should have been added to the map,
						// so we can safely assert that it will be present
						const outputFile = outputMap.get(uncommitedFile.path)!;
						if (!outputFile.conflicted) {
							outputFile.status = 'M';
						}
						return;
					}

					return;
				}

				outputMap.set(uncommitedFile.path, {
					name: uncommitedFile.filename,
					path: uncommitedFile.path,
					conflicted: false,
					status: 'A'
				});
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

	async function abort() {
		modeServiceAborting = 'loading';

		await modeService.abortEditAndReturnToWorkspace();

		modeServiceAborting = 'completed';
	}

	async function save() {
		modeServiceSaving = 'loading';

		await modeService.saveEditAndReturnToWorkspace();

		modeServiceSaving = 'completed';
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
			<h3 class="text-13 text-semibold commit-card__title">
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
			<div class="header">
				<h3 class="text-13 text-semibold">Commit files</h3>
				<Badge label={files.length} />
			</div>
			<ScrollableContainer>
				{#each files as file (file.path)}
					<div class="file">
						<FileListItem
							filePath={file.path}
							fileStatus={file.status}
							conflicted={file.conflicted}
							conflictHint={file.conflictHint}
							fileStatusStyle={file.status === 'M' ? 'full' : 'dot'}
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
		branchId={undefined}
	/>

	<p class="text-12 text-body editmode__helptext">
		Please don't make any commits while in edit mode.
		<br />
		To exit edit mode, use the provided actions.
	</p>

	<div class="editmode__actions">
		<Button style="ghost" outline onclick={abort} disabled={modeServiceAborting === 'loading'}>
			Cancel
		</Button>
		{#if conflictedFiles.length > 0}
			<Button
				style="neutral"
				kind="solid"
				onclick={openAllConflictedFiles}
				icon="open-link"
				tooltip={conflictedFiles.length === 1
					? 'Open the conflicted file in your editor'
					: 'Open all files with conflicts in your editor'}
			>
				Open conflicted files
			</Button>
		{/if}
		<Button
			style="pop"
			kind="solid"
			icon="tick-small"
			onclick={save}
			disabled={modeServiceSaving === 'loading'}
		>
			Save and exit
		</Button>
	</div>
</div>

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
