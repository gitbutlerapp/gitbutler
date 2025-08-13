<script lang="ts">
	import { goto } from '$app/navigation';
	import InfoMessage, { type MessageStyle } from '$components/InfoMessage.svelte';
	import Section from '$components/Section.svelte';
	import { POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { BACKEND } from '$lib/backend';
	import { GIT_SERVICE } from '$lib/git/gitService';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { projectPath } from '$lib/routes/routes.svelte';
	import { parseRemoteUrl } from '$lib/url/gitUrl';
	import { inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { Button, Spacer, Textbox } from '@gitbutler/ui';

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
		if (error instanceof Error) return error.message;

		if (
			typeof error === 'object' &&
			error !== null &&
			'message' in error &&
			typeof error.message === 'string'
		) {
			return error.message;
		}

		return String(error);
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

			posthog.capture('Repository Cloned', { protocol: remoteUrl.protocol });
			const project = await projectsService.addProject(targetDir);
			if (!project) {
				throw new Error('Failed to add project after cloning.');
			}
			goto(projectPath(project.id));
		} catch (e) {
			Sentry.captureException(e);
			const errorMessage = getErrorMessage(e);
			posthog.capture('Repository Clone Failure', { error: errorMessage });
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

<h1 class="clone-title text-serif-40">Clone a repository</h1>
<Section>
	<div class="clone__field repositoryUrl">
		<div class="text-13 text-semibold clone__field--label">Clone URL</div>
		<Textbox bind:value={repositoryUrl} />
	</div>
	<div class="clone__field repositoryTargetPath">
		<div class="text-13 text-semibold clone__field--label">Where to clone</div>
		<Textbox bind:value={targetDirPath} placeholder="/Users/tipsy/Documents" />
		<Button kind="outline" disabled={loading} onclick={handleCloneTargetSelect}>Choose..</Button>
	</div>
</Section>

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
						{@html item.label}
					{/each}
				{/if}
			{/snippet}
		</InfoMessage>
	</div>
{/snippet}

<style>
	.clone-title {
		margin-bottom: 20px;
		color: var(--clr-scale-ntrl-0);
		line-height: 1;
	}

	.clone__field {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.clone__field--label {
		color: var(--clr-scale-ntrl-50);
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
