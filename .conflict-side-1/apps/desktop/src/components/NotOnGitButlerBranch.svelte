<script lang="ts">
	import { goto } from '$app/navigation';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import ProjectNameLabel from '$components/ProjectNameLabel.svelte';
	import ProjectSwitcher from '$components/ProjectSwitcher.svelte';
	import RemoveProjectButton from '$components/RemoveProjectButton.svelte';
	import derectionDoubtSvg from '$lib/assets/illustrations/direction-doubt.svg?raw';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { VirtualBranchService } from '$lib/branches/virtualBranchService';
	import { ModeService } from '$lib/mode/modeService';
	import { showError } from '$lib/notifications/toasts';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { inject } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import FileListItem from '@gitbutler/ui/file/FileListItem.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';
	import * as toasts from '@gitbutler/ui/toasts';
	import type { BaseBranch } from '$lib/baseBranch/baseBranch';

	interface Props {
		baseBranch: BaseBranch;
	}

	const { baseBranch }: Props = $props();

	const projectsService = getContext(ProjectsService);
	const vbranchService = getContext(VirtualBranchService);
	const baseBranchService = getContext(BaseBranchService);
	const project = getContext(Project);
	const [setBaseBranchTarget, targetBranchSwitch] = baseBranchService.setTarget;

	const modeService = getContext(ModeService);
	// TODO: On filesystem change this should be reloaded
	const mode = modeService.mode;

	const [worktreeService] = inject(WorktreeService);
	const changes = worktreeService.treeChanges(project.id);

	let modal = $state<Modal>();

	async function switchTarget(branch: string, remote?: string, stashUncommitted?: boolean) {
		await setBaseBranchTarget({
			projectId: project.id,
			branch,
			pushRemote: remote,
			stashUncommitted
		});
		await vbranchService.refresh();
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

	let selectedHandlingOfUncommitted = $state('Stash');
	let doStash = $derived(selectedHandlingOfUncommitted === 'Stash');

	let handlingOptions: { label: string; value: string; selectable: boolean }[] = $derived([
		{
			label: 'Stash',
			value: 'Stash',
			selectable: true
		},
		{
			label: 'Bring to Workspace',
			value: 'Bring to Workspace',
			selectable: !conflicts // TODO: Reactivity??
		}
	]);

	async function initSwithToWorkspace() {
		if (changes.current.data?.length === 0) {
			switchTarget(baseBranch.branchName);
		} else {
			modal?.show();
		}
	}
</script>

<DecorativeSplitView img={derectionDoubtSvg}>
	<div class="switchrepo">
		<div class="project-name">
			<ProjectNameLabel projectName={project?.title} />
		</div>
		<p class="switchrepo__title text-18 text-body text-bold">
			Looks like you've switched away from <span class="code-string"> gitbutler/workspace </span>
		</p>

		<p class="switchrepo__message text-13 text-body">
			Due to GitButler managing multiple virtual branches, you cannot switch back and forth between
			git branches and virtual branches easily.
			<Link href="https://docs.gitbutler.com/features/virtual-branches/integration-branch">
				Learn more
			</Link>
		</p>

		<div class="switchrepo__actions">
			<AsyncButton
				style="pop"
				icon="undo-small"
				reversedDirection
				loading={targetBranchSwitch.current.isLoading}
				action={initSwithToWorkspace}
			>
				Switch to gitbutler/workspace
			</AsyncButton>

			{#if project}
				<RemoveProjectButton
					bind:this={deleteConfirmationModal}
					projectTitle={project.title}
					{isDeleting}
					{onDeleteClicked}
				/>
			{/if}
		</div>

		<Spacer dotted margin={0} />

		<div class="switchrepo__project">
			<ProjectSwitcher />
		</div>
	</div>

	<Modal
		bind:this={modal}
		width={520}
		noPadding
		onClose={() => {}}
		onSubmit={() => switchTarget(baseBranch.branchName, undefined, doStash)}
	>
		<div class="content-wrap text-13">
			<h1 class="text-15 text-bold">It looks like there are uncommitted changes</h1>
			<p>The following are uncommitted files in your worktree:</p>
			<div class="file-list">
				{#each changes.current.data || [] as change}
					<FileListItem filePath={change.path} />
				{/each}
			</div>
			{#if conflicts}
				<div>
					The following are uncommitted files that can't be applied to the workspace due to
					conflicts:
				</div>
				<div class="file-list">
					{#if $mode?.type === 'OutsideWorkspace'}
						{#each $mode.subject?.worktreeConflicts || [] as path}
							<FileListItem filePath={path} />
						{/each}
					{/if}
				</div>
			{/if}

			<p>What would you like to do with the files?</p>
			<Select
				value={selectedHandlingOfUncommitted}
				options={handlingOptions}
				onselect={(value) => {
					selectedHandlingOfUncommitted = value;
				}}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem
						disabled={!item.selectable}
						selected={item.value === selectedHandlingOfUncommitted}
						{highlighted}
					>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		</div>
		{#snippet controls(close)}
			<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
			<Button style="pop" type="submit">Confirm</Button>
		{/snippet}
	</Modal>
</DecorativeSplitView>

<style lang="postcss">
	.project-name {
		margin-bottom: 12px;
	}

	.switchrepo__title {
		margin-bottom: 12px;
		color: var(--clr-scale-ntrl-30);
	}

	.switchrepo__message {
		margin-bottom: 20px;
		color: var(--clr-scale-ntrl-50);
	}
	.switchrepo__actions {
		display: flex;
		flex-wrap: wrap;
		padding-bottom: 24px;
		gap: 8px;
	}

	.switchrepo__project {
		padding-top: 24px;
	}

	.content-wrap {
		display: flex;
		flex-direction: column;
		padding: 16px 12px;
		gap: 16px;
	}

	.file-list {
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
</style>
