<script lang="ts">
	import { goto } from '$app/navigation';
	import Chrome from '$components/Chrome.svelte';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import ProjectNameLabel from '$components/ProjectNameLabel.svelte';
	import ProjectSwitcher from '$components/ProjectSwitcher.svelte';
	import RemoveProjectButton from '$components/RemoveProjectButton.svelte';
	import { POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import loadErrorSvg from '$lib/assets/illustrations/load-error.svg?raw';
	import { showError } from '$lib/notifications/toasts';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/core/context';

	import { Icon, Spacer, chipToasts } from '@gitbutler/ui';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { onMount } from 'svelte';

	type Props = {
		projectId: string;
		projectTitle?: string;
		error?: any;
	};

	const { projectId, projectTitle, error = undefined }: Props = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);

	let loading = $state(false);
	let deleteConfirmationModal: ReturnType<typeof RemoveProjectButton> | undefined = $state();

	async function onDeleteClicked() {
		loading = true;
		try {
			deleteConfirmationModal?.close();
			await projectsService.deleteProject(projectId);
			chipToasts.success('Project deleted');
			goto('/');
		} catch (err: any) {
			console.error(err);
			showError('Failed to delete project', err);
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		posthog.capture('repo:load_failed', { error_message: String(error) });
	});
</script>

<Chrome {projectId} sidebarDisabled>
	<DecorativeSplitView img={loadErrorSvg}>
		<div class="problem">
			<div class="project-name">
				<ProjectNameLabel projectName={projectTitle} />
			</div>
			<h2 class="problem__title text-18 text-body text-bold">
				There was a problem loading this repo
			</h2>

			<div class="problem__error text-12 text-body">
				<Icon name="error" color="error" />
				{#if !isDefined(error)}
					'An unknown error occured'
				{:else if error instanceof Object && 'message' in error}
					{error.message}
				{:else}
					{error}
				{/if}
			</div>

			<div class="remove-project-btn">
				<RemoveProjectButton
					bind:this={deleteConfirmationModal}
					isDeleting={loading}
					{onDeleteClicked}
				/>
			</div>

			<Spacer dotted margin={0} />

			<div class="problem__switcher">
				<ProjectSwitcher {projectId} />
			</div>
		</div>
	</DecorativeSplitView>
</Chrome>

<style lang="postcss">
	.project-name {
		margin-bottom: 12px;
	}

	.problem__title {
		margin-bottom: 12px;
		color: var(--clr-scale-ntrl-30);
	}

	.problem__switcher {
		margin-top: 24px;
		text-align: right;
	}

	.problem__error {
		display: flex;
		margin-bottom: 12px;
		padding: 20px;
		gap: 12px;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-err-bg);
		color: var(--clr-scale-ntrl-0);
	}

	.remove-project-btn {
		display: flex;
		justify-content: flex-end;
		padding-bottom: 24px;
	}
</style>
