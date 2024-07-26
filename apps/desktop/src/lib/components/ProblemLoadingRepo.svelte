<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import RemoveProjectButton from './RemoveProjectButton.svelte';
	import loadErrorSvg from '$lib/assets/illustrations/load-error.svg?raw';
	import { ProjectService, Project } from '$lib/backend/projects';
	import { showError } from '$lib/notifications/toasts';
	import Icon from '$lib/shared/Icon.svelte';
	import { getContext } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { goto } from '$app/navigation';

	export let error: any = undefined;

	const projectService = getContext(ProjectService);
	const project = getContext(Project);

	let loading = false;
	let deleteConfirmationModal: RemoveProjectButton;

	async function onDeleteClicked() {
		loading = true;
		try {
			deleteConfirmationModal.close();
			await projectService.deleteProject(project.id);
			toasts.success('Project deleted');
			goto('/');
		} catch (err: any) {
			console.error(err);
			showError('Failed to delete project', err);
		} finally {
			loading = false;
			projectService.reload();
		}
	}
</script>

<DecorativeSplitView img={loadErrorSvg}>
	<div class="problem" data-tauri-drag-region>
		<p class="problem__project text-bold"><Icon name="repo-book" /> {project?.title}</p>
		<p class="problem__title text-base-body-18 text-bold" data-tauri-drag-region>
			There was a problem loading this repo
		</p>

		<div class="problem__error text-base-body-12">
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

		<div class="problem__switcher">
			<ProjectSwitcher />
		</div>
	</div>
</DecorativeSplitView>

<style lang="postcss">
	.problem__project {
		display: flex;
		gap: 8px;
		align-items: center;
		line-height: 120%;
		color: var(--clr-scale-ntrl-30);
		margin-bottom: 20px;
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
		border-bottom: 1px dashed var(--clr-scale-ntrl-60);
	}
</style>
