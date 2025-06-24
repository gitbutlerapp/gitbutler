<script lang="ts">
	import { goto } from '$app/navigation';
	import GitHubRepoPicker from '$components/GitHubRepoPicker.svelte';
	import InfoMessage, { type MessageStyle } from '$components/InfoMessage.svelte';
	import Section from '$components/Section.svelte';
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import { invoke } from '$lib/backend/ipc';
	import { ProjectsService } from '$lib/project/projectsService';
	import { ClientState } from '$lib/state/clientState.svelte';
	import { UserService } from '$lib/user/userService';
	import { parseRemoteUrl } from '$lib/url/gitUrl';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import * as Sentry from '@sentry/sveltekit';
	import { documentDir } from '@tauri-apps/api/path';
	import { join } from '@tauri-apps/api/path';
	import { open } from '@tauri-apps/plugin-dialog';
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import type { GitHubRepository } from '$lib/forge/github/githubRepoListService.svelte';

	const projectsService = getContext(ProjectsService);
	const userService = getContext(UserService);
	const posthog = getContext(PostHogWrapper);
	const clientState = getContext(ClientState);

	// Access GitHub API through client state
	const gitHubApi = clientState.githubApi;

	const user = userService.user;

	let loading = $state(false);
	let cloneProgress = $state<string>('');
	let errors = $state<{ label: string }[]>([]);
	let completed = $state(false);
	let repositoryUrl = $state('');
	let targetDirPath = $state('');
	let savedTargetDirPath = persisted('', 'clone_targetDirPath');

	// GitHub repo picker state
	let showRepoPicker = $state(false);
	let repoPicker = $state<GitHubRepoPicker>();

	// Check if user is authenticated with GitHub
	const isGitHubAuthenticated = $derived(!!$user?.github_access_token);

	function onRepoSelect(repoUrl: string) {
		repositoryUrl = repoUrl;
		showRepoPicker = false;
	}

	async function openGitHubPicker() {
		await repoPicker?.openModal();
	}

	onMount(async () => {
		if ($savedTargetDirPath) {
			targetDirPath = $savedTargetDirPath;
		} else {
			targetDirPath = await documentDir();
		}
	});

	async function handleCloneTargetSelect() {
		const selectedPath = await open({
			directory: true,
			recursive: true,
			title: 'Target Clone Directory'
		});
		if (!selectedPath || !selectedPath[0]) return;

		targetDirPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;
	}

	async function cloneRepository() {
		loading = true;
		cloneProgress = 'Preparing to clone...';
		savedTargetDirPath.set(targetDirPath);
		if (errors.length) {
			errors = [];
		}

		if (!repositoryUrl || !targetDirPath) {
			errors.push({
				label: 'You must add both a repository URL and target directory.'
			});
			loading = false;
			cloneProgress = '';
			return;
		}

		// Listen for progress events from the backend
		const unlisten = await listen<string>('clone_progress', (event) => {
			cloneProgress = event.payload;
		});

		try {
			cloneProgress = 'Parsing repository URL...';
			const remoteUrl = parseRemoteUrl(repositoryUrl);
			if (!remoteUrl) {
				return;
			}

			cloneProgress = 'Setting up target directory...';
			const targetDir = await join(targetDirPath, remoteUrl.name);

			// The backend will now emit progress events
			await invoke('git_clone_repository', {
				repositoryUrl,
				targetDir
			});

			cloneProgress = 'Finalizing project setup...';
			posthog.capture('Repository Cloned', { protocol: remoteUrl.protocol });
			await projectsService.addProject(targetDir);
			
			cloneProgress = 'Clone completed!';
		} catch (e) {
			Sentry.captureException(e);
			posthog.capture('Repository Clone Failure', { error: String(e) });
			errors.push({
				label: String(e)
			});
		} finally {
			loading = false;
			cloneProgress = '';
			unlisten();
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
		<div class="clone__field--input-container">
			<Textbox bind:value={repositoryUrl} placeholder="https://github.com/user/repo.git" />
			{#if isGitHubAuthenticated}
				<Button 
					kind="outline" 
					disabled={loading} 
					onclick={openGitHubPicker}
					icon="github"
				>
					Browse GitHub
				</Button>
			{/if}
		</div>
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

{#if cloneProgress}
	<div class="clone__progress">
		<div class="clone__progress-indicator">
			<Icon name="spinner" />
			<span class="text-13">{cloneProgress}</span>
		</div>
	</div>
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

<!-- GitHub Repository Picker Modal -->
{#if isGitHubAuthenticated}
	<GitHubRepoPicker 
		bind:this={repoPicker}
		{gitHubApi}
		onRepoSelected={onRepoSelect}
		onClose={() => {}}
	/>
{/if}

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

	.clone__field--input-container {
		display: flex;
		gap: 8px;
		align-items: stretch;
	}

	.clone__field--input-container :global(.textbox) {
		flex: 0 0 70%;
	}

	.clone__field--input-container :global(button) {
		flex: 0 0 30%;
		min-width: 0;
		height: auto;
	}

	.clone__progress {
		padding: 12px 0;
		border-bottom: 1px solid var(--clr-border-2);
		margin-bottom: 16px;
	}

	.clone__progress-indicator {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--clr-text-2);
	}

	.clone__progress-indicator :global(.icon) {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
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
