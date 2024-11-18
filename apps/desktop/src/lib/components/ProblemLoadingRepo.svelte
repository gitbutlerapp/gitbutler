<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import RemoveProjectButton from './RemoveProjectButton.svelte';
	import loadErrorSvg from '$lib/assets/illustrations/load-error.svg?raw';
	import { ProjectsService, Project } from '$lib/backend/projects';
	import { showError } from '$lib/notifications/toasts';
	import ProjectNameLabel from '$lib/shared/ProjectNameLabel.svelte';
	import * as toasts from '$lib/utils/toasts';
	import { getContext } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import { goto } from '$app/navigation';

	export let error: any = undefined;

	const projectsService = getContext(ProjectsService);
	const project = getContext(Project);

	let loading = false;
	let deleteConfirmationModal: ReturnType<typeof RemoveProjectButton> | undefined;

	async function onDeleteClicked() {
		loading = true;
		try {
			deleteConfirmationModal?.close();
			await projectsService.deleteProject(project.id);
			toasts.success('Project deleted');
			goto('/');
		} catch (err: any) {
			console.error(err);
			showError('Failed to delete project', err);
		} finally {
			loading = false;
			projectsService.reload();
		}
	}
</script>

<DecorativeSplitView img={loadErrorSvg}>
	<div class="problem">
		<div class="project-name">
			<ProjectNameLabel projectName={project?.title} />
		</div>
		<h2 class="problem__title text-18 text-body text-bold">
			There was a problem loading this repo
		</h2>

		<div class="problem__error text-12 text-body">
			<Icon name="error" color="error" />
			{error ? error : 'An unknown error occurred'}
		</div>

		<div class="remove-project-btn">
			<RemoveProjectButton
				bind:this={deleteConfirmationModal}
				projectTitle={project.title}
				isDeleting={loading}
				{onDeleteClicked}
			/>
		</div>

		<Spacer dotted margin={0} />

		<div class="problem__switcher">
			<ProjectSwitcher />
		</div>
	</div>
</DecorativeSplitView>

<style lang="postcss">
	.project-name {
		margin-bottom: 12px;
	}

	.problem__title {
		color: var(--clr-scale-ntrl-30);
		margin-bottom: 12px;
	}

	.problem__switcher {
		text-align: right;
		margin-top: 24px;
	}

	.problem__error {
		display: flex;
		color: var(--clr-scale-ntrl-0);
		gap: 12px;
		padding: 20px;
		background-color: var(--clr-theme-err-bg);
		border-radius: var(--radius-m);
		margin-bottom: 12px;
	}

	.remove-project-btn {
		display: flex;
		justify-content: flex-end;
		padding-bottom: 24px;
	}
</style>
