<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import RemoveProjectButton from './RemoveProjectButton.svelte';
	import ProjectNameLabel from '$components/ProjectNameLabel.svelte';
	import derectionDoubtSvg from '$lib/assets/illustrations/direction-doubt.svg?raw';
	import { BranchController } from '$lib/branches/branchController';
	import { showError } from '$lib/notifications/toasts';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import * as toasts from '@gitbutler/ui/toasts';
	import type { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { goto } from '$app/navigation';

	interface Props {
		baseBranch: BaseBranch;
	}

	const { baseBranch }: Props = $props();

	const branchController = getContext(BranchController);
	const projectsService = getContext(ProjectsService);
	const project = getContext(Project);

	let isDeleting = $state(false);
	let deleteConfirmationModal: ReturnType<typeof RemoveProjectButton> | undefined = $state();

	async function onDeleteClicked() {
		if (project) {
			isDeleting = true;
			try {
				deleteConfirmationModal?.close();
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
			<Button
				style="pop"
				icon="undo-small"
				reversedDirection
				onclick={() => {
					if (baseBranch) branchController.setTarget(baseBranch.branchName);
				}}
			>
				Go back to gitbutler/workspace
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

		<Spacer dotted margin={0} />

		<div class="switchrepo__project">
			<ProjectSwitcher />
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
	}

	.switchrepo__project {
		padding-top: 24px;
	}
</style>
