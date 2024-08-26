<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectNameLabel from '../shared/ProjectNameLabel.svelte';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import { Project } from '$lib/backend/projects';
	import { InitialFile, ModeService, type EditModeMetadata } from '$lib/modes/service';
	import { UncommitedFilesWatcher } from '$lib/uncommitedFiles/watcher';
	import { getContext } from '$lib/utils/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import type { FileStatus } from '@gitbutler/ui/file/types';

	interface Props {
		editModeMetadata: EditModeMetadata;
	}

	const { editModeMetadata }: Props = $props();

	const project = getContext(Project);
	const uncommitedFileWatcher = getContext(UncommitedFilesWatcher);
	const modeService = getContext(ModeService);

	const uncommitedFiles = uncommitedFileWatcher.uncommitedFiles;

	let modeServiceSaving = $state<'inert' | 'loading' | 'completed'>('inert');

	let initialFiles = $state<InitialFile[]>([]);

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
		const files: FileEntry[] = initialFiles.map((initialFile) => ({
			name: initialFile.filename,
			path: initialFile.filePath,
			conflicted: initialFile.conflicted,
			status: $uncommitedFiles.some(
				(uncommitedFile) => uncommitedFile[0].path === initialFile.filePath
			)
				? undefined
				: 'D'
		}));

		console.log(initialFiles);

		$uncommitedFiles.forEach((uncommitedFile) => {
			console.log(uncommitedFile);
			const foundFile = files.find((file) => file.path === uncommitedFile[0].path);

			if (foundFile) {
				const initialFile = initialFiles.find(
					(initialFile) => initialFile.filePath === foundFile.path
				)!;

				// This may incorrectly be true if the file is conflicted
				// To compensate for this, we also check if the uncommited diff
				// has conflict markers.
				const fileChanged = initialFile.file.hunks.some(
					(hunk) => !uncommitedFile[0].hunks.map((hunk) => hunk.diff).includes(hunk.diff)
				);

				if (fileChanged && !uncommitedFile[0].looksConflicted) {
					foundFile.status = 'M';
					foundFile.conflicted = false;
				}
			} else {
				files.push({
					name: uncommitedFile[0].filename,
					path: uncommitedFile[0].path,
					conflicted: false,
					status: 'A'
				});
			}
		});

		files.sort((a, b) => a.path.localeCompare(b.path));

		return files;
	});

	async function abort() {
		modeServiceSaving = 'loading';

		await modeService.abortEditAndReturnToWorkspace();

		modeServiceSaving = 'completed';
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
		<p class="switchrepo__title text-18 text-body text-bold">
			You are currently editing commit <span class="code-string">
				{editModeMetadata.commitOid.slice(0, 7)}
			</span>
		</p>

		<p class="switchrepo__message text-13 text-body">
			Please do not make any commits whilst in edit mode. To leave edit mode, use the provided
			actions.
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
						fileStatusStyle={'full'}
					/>
				</div>
			{/each}
		</div>

		<div class="switchrepo__actions">
			<Button style="ghost" outline onclick={abort} loading={modeServiceSaving === 'loading'}>
				Cancel changes
			</Button>
			<Button
				style="pop"
				kind="solid"
				icon="undo-small"
				onclick={save}
				loading={modeServiceSaving === 'loading'}
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
		color: var(--clr-scale-ntrl-30);
		margin-bottom: 12px;
	}

	.switchrepo__message {
		color: var(--clr-scale-ntrl-50);
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
