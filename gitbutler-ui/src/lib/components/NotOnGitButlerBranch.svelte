<script lang="ts">
	import Button from './Button.svelte';
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import Link from './Link.svelte';
	import ProjectNameLabel from './ProjectNameLabel.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import RemoveProjectButton from './RemoveProjectButton.svelte';
	import derectionDoubtSvg from '$lib/assets/illustrations/direction-doubt.svg?raw';
	import { ProjectService, Project } from '$lib/backend/projects';
	import { getContext } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch } from '$lib/vbranches/types';
	import { goto } from '$app/navigation';

	export let baseBranch: BaseBranch;

	const branchController = getContext(BranchController);
	const projectService = getContext(ProjectService);
	const project = getContext(Project);

	let isDeleting = false;
	let deleteConfirmationModal: RemoveProjectButton;

	async function onDeleteClicked() {
		if (project) {
			isDeleting = true;
			try {
				deleteConfirmationModal.close();
				await projectService.deleteProject(project.id);
				toasts.success('Project deleted');
				goto('/', { invalidateAll: true });
			} catch (e) {
				console.error(e);
				toasts.error('Failed to delete project');
			} finally {
				isDeleting = false;
				projectService.reload();
			}
		}
	}
</script>

<DecorativeSplitView img={derectionDoubtSvg}>
	<div class="switchrepo">
		<div class="project-name">
			<ProjectNameLabel projectName={project?.title} />
		</div>
		<p class="switchrepo__title text-base-body-18 text-bold">
			Looks like you've switched away from <span class="code-string"> gitbutler/integration </span>
		</p>

		<p class="switchrepo__message text-base-body-13">
			Due to GitButler managing multiple virtual branches, you cannot switch back and forth between
			git branches and virtual branches easily.
			<Link href="https://docs.gitbutler.com/features/virtual-branches/integration-branch">
				Learn more
			</Link>
		</p>

		<div class="switchrepo__actions">
			<Button
				color="primary"
				icon="chevron-left-small"
				iconAlign="left"
				on:click={() => {
					if (baseBranch) branchController.setTarget(baseBranch.branchName);
				}}
			>
				Go back to gitbutler/integration
			</Button>

			{#if project}
				<RemoveProjectButton
					bind:this={deleteConfirmationModal}
					projectTitle={project.title}
					{isDeleting}
					{onDeleteClicked}
				/>
			{/if}
		</div>

		<div class="switchrepo__project">
			<ProjectSwitcher />
		</div>
	</div>
</DecorativeSplitView>

<style lang="postcss">
	.project-name {
		display: flex;
		gap: var(--size-8);
		align-items: center;
		line-height: 120%;
		color: var(--clr-theme-scale-ntrl-30);
		margin-bottom: var(--size-20);
	}

	.switchrepo__title {
		color: var(--clr-theme-scale-ntrl-30);
		margin-bottom: var(--size-12);
	}

	.switchrepo__message {
		color: var(--clr-theme-scale-ntrl-50);
		margin-bottom: var(--size-20);
	}
	.switchrepo__actions {
		display: flex;
		gap: var(--size-8);
		padding-bottom: var(--size-24);
		flex-wrap: wrap;
	}

	.switchrepo__project {
		padding-top: var(--size-24);
		border-top: 1px dashed var(--clr-theme-scale-ntrl-60);
	}
</style>
