<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectNameLabel from '../shared/ProjectNameLabel.svelte';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import { Project } from '$lib/backend/projects';
	import { ModeService, type EditModeMetadata } from '$lib/modes/service';
	import { UncommitedFilesWatcher } from '$lib/uncommitedFiles/watcher';
	import { getContext } from '$lib/utils/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import type { RemoteFile } from '$lib/vbranches/types';
	import type { FileStatus } from '@gitbutler/ui/file/types';

	interface Props {
		editModeMetadata: EditModeMetadata;
	}

	const { editModeMetadata }: Props = $props();

	const project = getContext(Project);
	const uncommitedFileWatcher = getContext(UncommitedFilesWatcher);
	const modeService = getContext(ModeService);

	const uncommitedFiles = uncommitedFileWatcher.uncommitedFiles;

	let modeServiceAborting = $state<'inert' | 'loading' | 'completed'>('inert');
	let modeServiceSaving = $state<'inert' | 'loading' | 'completed'>('inert');

	let initialFiles = $state<RemoteFile[]>([]);

	$effect(() => {
		modeService.getInitialIndexState().then((files) => {
			initialFiles = files;
		});
	});

	interface FileEntry {
		name: string;
		path: string;
		conflicted: boolean;
		status?: FileStatus;
	}

	const files = $derived.by(() => {
		const initialFileMap = new Map<string, RemoteFile>();
		const uncommitedFileMap = new Map<string, RemoteFile>();
		const outputMap = new Map<string, FileEntry>();

		// Build maps of files
		{
			initialFiles.forEach((initialFile) => {
				initialFileMap.set(initialFile.path, initialFile);
			});

			$uncommitedFiles.forEach(([uncommitedFile]) => {
				uncommitedFileMap.set(uncommitedFile.path, uncommitedFile);
			});
		}

		// Create output
		{
			initialFiles.forEach((initialFile) => {
				const isDeleted = uncommitedFileMap.has(initialFile.path);

				outputMap.set(initialFile.path, {
					name: initialFile.filename,
					path: initialFile.path,
					conflicted: initialFile.looksConflicted,
					status: isDeleted ? undefined : 'D'
				});
			});

			$uncommitedFiles.forEach(([uncommitedFile]) => {
				const initialFile = initialFileMap.get(uncommitedFile.path);
				if (initialFile) {
					const fileChanged = initialFile.hunks.some(
						(hunk) => !uncommitedFile.hunks.map((hunk) => hunk.diff).includes(hunk.diff)
					);

					if (fileChanged && !uncommitedFile.looksConflicted) {
						// All initial entries should have been added to the map,
						// so we can safly assert that it will be present
						const outputFile = outputMap.get(uncommitedFile.path)!;
						outputFile.status = 'M';
						outputFile.conflicted = false;
					}
				} else {
					outputMap.set(uncommitedFile.path, {
						name: uncommitedFile.filename,
						path: uncommitedFile.path,
						conflicted: false,
						status: 'A'
					});
				}
			});
		}

		const files = Array.from(outputMap.values());
		files.sort((a, b) => a.path.localeCompare(b.path));

		return files;
	});

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
</script>

<DecorativeSplitView img={newProjectSvg}>
	<div class="switchrepo">
		<div class="project-name">
			<ProjectNameLabel projectName={project?.title} />
		</div>
		<h2 class="switchrepo__title text-18 text-body text-bold">
			You are currently editing commit <span class="code-string">
				{editModeMetadata.commitOid.slice(0, 7)}
			</span>
		</h2>

		<p class="switchrepo__message text-12 text-body">
			Please do not make any commits whilst in edit mode.
			<br />
			To leave edit mode, use the provided actions.
		</p>

		<div class="files">
			<p class="text-13 text-semibold header">Commit files</p>
			{#each files as file}
				<div class="file">
					<FileListItem
						fileName={file.name}
						filePath={file.path}
						fileStatus={file.status}
						conflicted={file.conflicted}
						fileStatusStyle="full"
						clickable={false}
					/>
				</div>
			{/each}
		</div>

		<div class="switchrepo__actions">
			<Button style="ghost" outline onclick={abort} disabled={modeServiceAborting === 'loading'}>
				Cancel changes
			</Button>
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
</DecorativeSplitView>

<style lang="postcss">
	.project-name {
		margin-bottom: 12px;
	}

	.switchrepo__title {
		color: var(--clr-text-1);
		margin-bottom: 12px;
	}

	.switchrepo__message {
		color: var(--clr-text-2);
		margin-bottom: 20px;
	}
	.switchrepo__actions {
		display: flex;
		gap: 8px;
		padding-bottom: 24px;
		flex-wrap: wrap;
		justify-content: flex-end;
	}

	.files {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);

		margin-bottom: 16px;

		overflow: hidden;

		padding-bottom: 8px;

		& .header {
			margin-left: 16px;
			margin-top: 16px;
			margin-bottom: 8px;
		}

		& .file {
			border-bottom: 1px solid var(--clr-border-3);
			&:last-child {
				border-bottom: none;
			}
		}
	}
</style>
