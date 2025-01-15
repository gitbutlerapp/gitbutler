<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import RemoveProjectButton from './RemoveProjectButton.svelte';
	import ProjectNameLabel from '$components/ProjectNameLabel.svelte';
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import loadErrorSvg from '$lib/assets/illustrations/load-error.svg?raw';
	import { showError } from '$lib/notifications/toasts';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import * as toasts from '$lib/utils/toasts';
	import { getContext } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	interface Props {
		error?: any;
	}

	const { error = undefined }: Props = $props();

	const projectsService = getContext(ProjectsService);
	const posthog = getContext(PostHogWrapper);
	const project = getContext(Project);

	let loading = $state(false);
	let deleteConfirmationModal: ReturnType<typeof RemoveProjectButton> | undefined = $state();

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

	onMount(() => {
		posthog.capture('repo:load_failed', { error_message: String(error) });
	});
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
