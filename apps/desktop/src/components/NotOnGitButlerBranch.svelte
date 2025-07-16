<script lang="ts">
	import { goto } from '$app/navigation';
	import Chrome from '$components/Chrome.svelte';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import RemoveProjectButton from '$components/RemoveProjectButton.svelte';
	import directionDoubtSvg from '$lib/assets/illustrations/direction-doubt.svg?raw';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import { showError } from '$lib/notifications/toasts';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { inject } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import RadioButton from '@gitbutler/ui/RadioButton.svelte';
	import FileListItem from '@gitbutler/ui/file/FileListItemV3.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import * as toasts from '@gitbutler/ui/toasts';
	import type { BaseBranch } from '$lib/baseBranch/baseBranch';

	type OptionsType = 'stash' | 'bring-to-workspace';

	interface Props {
		baseBranch: BaseBranch;
	}

	const { baseBranch }: Props = $props();

	const projectsService = getContext(ProjectsService);
	const baseBranchService = getContext(BaseBranchService);
	const project = getContext(Project);
	const [setBaseBranchTarget, targetBranchSwitch] = baseBranchService.setTarget;

	const modeService = getContext(ModeService);
	// TODO: On filesystem change this should be reloaded
	const mode = modeService.mode;

	const [worktreeService] = inject(WorktreeService);
	const changes = worktreeService.treeChanges(project.id);

	async function switchTarget(branch: string, remote?: string, stashUncommitted?: boolean) {
		await setBaseBranchTarget({
			projectId: project.id,
			branch,
			pushRemote: remote,
			stashUncommitted
		});
	}

	let isDeleting = $state(false);
	let deleteConfirmationModal: ReturnType<typeof RemoveProjectButton> | undefined = $state();

	async function onDeleteClicked() {
		if (project) {
			isDeleting = true;
			try {
				await projectsService.deleteProject(project.id);
				toasts.success('Project deleted');
				goto('/', { invalidateAll: true });
			} catch (err: any) {
				console.error(err);
				showError('Failed to delete project', err);
			} finally {
				isDeleting = false;
				projectsService.reload();
			}
		}
	}

	let conflicts = $derived(
		$mode?.type === 'OutsideWorkspace' && $mode.subject?.worktreeConflicts.length > 0
	);

	let selectedHandlingOfUncommitted: OptionsType = $state('stash');
	let doStash = $derived(selectedHandlingOfUncommitted === 'stash');

	let handlingOptions: { label: string; value: OptionsType; selectable: boolean }[] = $derived([
		{
			label: 'Stash',
			value: 'stash',
			selectable: true
		},
		{
			label: 'Bring to Workspace',
			value: 'bring-to-workspace',
			selectable: !conflicts // TODO: Reactivity??
		}
	]);

	async function initSwithToWorkspace() {
		if (changes.current.data?.length === 0) {
			switchTarget(baseBranch.branchName);
		} else {
			switchTarget(baseBranch.branchName, undefined, doStash);
		}
	}
</script>

<Chrome projectId={project.id} sidebarDisabled>
	<DecorativeSplitView img={directionDoubtSvg} hideDetails>
		{@const uncommittedChanges = changes.current.data || []}

		<div class="switchrepo__content">
			<p class="switchrepo__title text-18 text-body text-bold">
				You've switched away from <span class="code-string"> gitbutler/workspace </span>
			</p>

			<p class="switchrepo__message text-13 text-body">
				Due to GitButler managing multiple virtual branches, you cannot switch back and forth
				between git branches and virtual branches easily.
				<Link href="https://docs.gitbutler.com/features/virtual-branches/integration-branch">
					Learn more
				</Link>
			</p>

			{#if uncommittedChanges.length > 0}
				<div class="switchrepo__uncommited-changes">
					<div class="switchrepo__uncommited-changes__section">
						<p class="switchrepo__label text-13 text-body text-bold">
							You have uncommitted changes:
						</p>
						<div class="switchrepo__file-list">
							{#each uncommittedChanges as change}
								<FileListItem
									filePath={change.path}
									clickable={false}
									isLast={change === uncommittedChanges[uncommittedChanges.length - 1]}
								/>
							{/each}
						</div>
						{#if conflicts}
							<p class="switchrepo__label text-13 text-body clr-text-2">
								Some files can’t be applied due to conflicts:
							</p>
							<div class="switchrepo__file-list">
								{#if $mode?.type === 'OutsideWorkspace'}
									{#each $mode.subject?.worktreeConflicts || [] as path}
										<FileListItem
											filePath={path}
											clickable={false}
											conflicted
											conflictHint="Resolve to apply"
											isLast={path ===
												$mode.subject?.worktreeConflicts[
													$mode.subject?.worktreeConflicts.length - 1
												]}
										/>
									{/each}
								{/if}
							</div>
						{/if}
					</div>

					<hr class="switchrepo__divider" />

					<p class="switchrepo__label text-13 text-body text-bold">
						What should we do with your uncommitted changes?
					</p>

					<div class="switchrepo__handling-options">
						{#each handlingOptions as item (item.value)}
							<label for={item.value} class="switchrepo__handling-options__item">
								<RadioButton
									id={item.value}
									value={item.value}
									onchange={() => {
										selectedHandlingOfUncommitted = item.value as OptionsType;
										doStash = selectedHandlingOfUncommitted === 'stash';
									}}
									checked={selectedHandlingOfUncommitted === item.value}
								/>
								<p class="text-13 text-body">{item.label}</p>
							</label>
						{/each}
					</div>
				</div>
			{/if}

			<div class="switchrepo__actions">
				<AsyncButton
					style="pop"
					icon="arrow-left"
					reversedDirection
					loading={targetBranchSwitch.current.isLoading}
					action={initSwithToWorkspace}
				>
					Switch back
				</AsyncButton>

				{#if project}
					<RemoveProjectButton
						bind:this={deleteConfirmationModal}
						outlineStyle
						projectTitle={project.title}
						{isDeleting}
						{onDeleteClicked}
					/>
				{/if}
			</div>
		</div>
	</DecorativeSplitView>
</Chrome>

<style lang="postcss">
	.switchrepo__title {
		margin-bottom: 12px;
		color: var(--clr-text-1);
	}

	.switchrepo__message {
		margin-bottom: 20px;
		color: var(--clr-text-2);
	}

	.switchrepo__content {
		display: flex;
		flex-direction: column;
	}

	.switchrepo__uncommited-changes {
		margin-bottom: 20px;
		padding: 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.switchrepo__divider {
		margin: 20px -16px;
		border: 0;
		border-top: 1px solid var(--clr-border-2);
	}

	.switchrepo__label {
		margin-bottom: 12px;
	}

	.switchrepo__file-list {
		margin-bottom: 16px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.switchrepo__handling-options {
		display: flex;
		gap: 20px;
	}

	.switchrepo__handling-options__item {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
	}

	.switchrepo__actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		width: 100%;
		padding-bottom: 24px;
		gap: 8px;
	}
</style>
