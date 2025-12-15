<script lang="ts">
	import { goto } from '$app/navigation';
	import SettingsSection from '$components/SettingsSection.svelte';
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { BACKEND } from '$lib/backend';
	import { parseError } from '$lib/error/parser';
	import { GIT_SERVICE } from '$lib/git/gitService';
	import { handleAddProjectOutcome } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { projectPath } from '$lib/routes/routes.svelte';
	import { parseRemoteUrl } from '$lib/url/gitUrl';
	import { inject } from '@gitbutler/core/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { Button, InfoMessage, type MessageStyle, Spacer, Textbox } from '@gitbutler/ui';

	import * as Sentry from '@sentry/sveltekit';
	import { onMount } from 'svelte';

	const projectsService = inject(PROJECTS_SERVICE);
	const gitService = inject(GIT_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);
	const backend = inject(BACKEND);

	let loading = $state(false);
	let errors = $state<{ label: string }[]>([]);
	let completed = $state(false);
	let repositoryUrl = $state('');
	let targetDirPath = $state('');
	let savedTargetDirPath = persisted('', 'clone_targetDirPath');

	onMount(async () => {
		if ($savedTargetDirPath) {
			targetDirPath = $savedTargetDirPath;
		} else {
			targetDirPath = await backend.documentDir();
		}
	});

	async function handleCloneTargetSelect() {
		const selectedPath = await backend.filePicker({
			directory: true,
			recursive: true,
			title: 'Target Clone Directory'
		});
		if (!selectedPath || !selectedPath[0]) return;

		targetDirPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;
	}

	function getErrorMessage(error: unknown): string {
		const parsedError = parseError(error);
		if (parsedError.name && parsedError.name !== parsedError.message) {
			return `${parsedError.name}: ${parsedError.message}`;
		}
		return parsedError.message;
	}

	async function cloneRepository() {
		loading = true;
		savedTargetDirPath.set(targetDirPath);
		if (errors.length) {
			errors = [];
		}

		if (!repositoryUrl || !targetDirPath) {
			errors.push({
				label: 'You must add both a repository URL and target directory.'
			});
			loading = false;
			return;
		}

		try {
			const remoteUrl = parseRemoteUrl(repositoryUrl);
			if (!remoteUrl) {
				return;
			}

			const targetDir = await backend.joinPath(targetDirPath, remoteUrl.name);

			await gitService.cloneRepo(repositoryUrl, targetDir);

			posthog.captureOnboarding(OnboardingEvent.ClonedProject);
			const outcome = await projectsService.addProject(targetDir);
			if (!outcome) {
				posthog.captureOnboarding(
					OnboardingEvent.ClonedProjectFailed,
					'Failed to add project after cloning'
				);
				throw new Error('Failed to add project after cloning.');
			}

			handleAddProjectOutcome(outcome, (project) => goto(projectPath(project.id)));
		} catch (e) {
			Sentry.captureException(e);
			const errorMessage = getErrorMessage(e);
			posthog.captureOnboarding(OnboardingEvent.ClonedProjectFailed, e);
			errors.push({
				label: errorMessage
			});
		} finally {
			loading = false;
		}
	}

	function handleCancel() {
		if (history.length > 0) {
			history.back();
		} else {
			goto('/');
		}
	}
</script>

<h1 class="clone-title text-serif-42">Clone a <i>repository</i></h1>
<SettingsSection>
	<Textbox label="Clone URL" bind:value={repositoryUrl} />

	<div class="clone__field repositoryTargetPath">
		<Textbox
			label="Where to clone"
			bind:value={targetDirPath}
			placeholder="/Users/tipsy/Documents"
		/>
		<Button kind="outline" disabled={loading} onclick={handleCloneTargetSelect}>Choose..</Button>
	</div>
</SettingsSection>

<Spacer dotted margin={24} />

{#if completed}
	{@render Notification({ title: 'Success', style: 'success' })}
{/if}
{#if errors.length}
	{@render Notification({ title: 'Error', items: errors, style: 'error' })}
{/if}

<div class="clone__actions">
	<Button kind="outline" disabled={loading} onclick={handleCancel}>Cancel</Button>
	<Button
		style="pop"
		icon={errors.length > 0 ? 'update' : 'chevron-right-small'}
		disabled={loading}
		{loading}
		onclick={cloneRepository}
	>
		{#if loading}
			Cloning..
		{:else if errors.length > 0}
			Retry clone
		{:else}
			Clone
		{/if}
	</Button>
</div>

{#snippet Notification({
	title: titleLabel,
	items,
	style
}: {
	title: string;
	items?: any[];
	style: MessageStyle;
})}
	<div class="clone__info-message">
		<InfoMessage {style} filled outlined={false}>
			{#snippet title()}
				{titleLabel}
			{/snippet}
			{#snippet content()}
				{#if items && items.length > 0}
					{#each items as item}
						<span>{item.label}</span>
					{/each}
				{/if}
			{/snippet}
		</InfoMessage>
	</div>
{/snippet}

<style>
	.clone-title {
		margin-bottom: 20px;
		color: var(--clr-text-1);
		line-height: 1;
	}

	.clone__field {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.clone__actions {
		display: flex;
		justify-content: end;
		gap: 8px;
	}

	.clone__info-message {
		margin-bottom: 20px;
	}
</style>
