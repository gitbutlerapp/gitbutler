<script lang="ts">
	import { ProjectService } from '$lib/backend/projects';
	import { persisted } from '$lib/persisted/persisted';
	import Section from '$lib/settings/Section.svelte';
	import Button from '$lib/shared/Button.svelte';
	import InfoMessage, { type MessageStyle } from '$lib/shared/InfoMessage.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import { parseRemoteUrl } from '$lib/url/gitUrl';
	import { getContext } from '$lib/utils/context';
	import Segment from '@gitbutler/ui/SegmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/SegmentControl/SegmentControl.svelte';
	import { open } from '@tauri-apps/api/dialog';
	import { documentDir } from '@tauri-apps/api/path';
	import { Command } from '@tauri-apps/api/shell';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	const projectService = getContext(ProjectService);

	const SSH_URL_PLACEHOLDER = 'ssh://';
	const HTTP_URL_PLACEHOLDER = 'https://';

	const RemoteType = {
		url: 'url',
		ssh: 'ssh'
	} as const;

	let loading = $state(false);
	let errors = $state<{ label: string }[]>([]);
	let completed = $state(false);
	let repositoryUrl = $state('');
	let targetDirPath = $state('');
	// TODO: Fix types
	let remoteType = $state<string | keyof typeof RemoteType>(RemoteType.url);
	let savedTargetDirPath = persisted('', 'clone_targetDirPath');

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
		if (!selectedPath) return;

		targetDirPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;
	}

	async function cloneRepository() {
		loading = true;
		savedTargetDirPath.set(targetDirPath);
		clearNotifications();

		if (!repositoryUrl || !targetDirPath) {
			errors.push({
				label: 'You must add both a repository URL and target directory.'
			});
			loading = false;
			return;
		}

		const { name } = parseRemoteUrl(repositoryUrl);
		try {
			// TODO: Get rust folks to implement a 'clone' fn to invoke :)
			await new Command('git', ['clone', repositoryUrl, `${targetDirPath}/${name}`]).execute();

			await projectService.addProject(`${targetDirPath}/${name}`);
		} catch (e) {
			errors.push({
				label: String(e)
			});
		} finally {
			loading = false;
		}
	}

	function handleRemoteTypeToggle(id: keyof typeof RemoteType | string) {
		function isEmpty(value: string) {
			if (!value) return true;
			if ([SSH_URL_PLACEHOLDER, HTTP_URL_PLACEHOLDER].includes(value)) return true;
			return false;
		}

		remoteType = id;

		if (id === RemoteType.ssh && isEmpty(repositoryUrl)) {
			repositoryUrl = SSH_URL_PLACEHOLDER;
		} else if (id === RemoteType.url && isEmpty(repositoryUrl)) {
			repositoryUrl = HTTP_URL_PLACEHOLDER;
		}
	}

	function handleCancel() {
		if (history.length > 0) {
			history.back();
		} else {
			goto('/');
		}
	}

	function clearNotifications() {
		if (errors.length) {
			errors = [];
		}
	}
</script>

<h1 class="clone-title text-serif-40">Clone a repository</h1>
<Section>
	<div class="clone__remoteType">
		<fieldset name="remoteType" class="remoteType-group">
			<SegmentControl fullWidth defaultIndex={0} onselect={handleRemoteTypeToggle}>
				<Segment id="url">URL</Segment>
				<Segment id="ssh">SSH</Segment>
			</SegmentControl>
		</fieldset>
	</div>
	<div class="clone__field repositoryUrl">
		<TextBox
			bind:value={repositoryUrl}
			placeholder={remoteType === 'url' ? HTTP_URL_PLACEHOLDER : SSH_URL_PLACEHOLDER}
			helperText={remoteType === 'url' ? 'Clone using the web URL' : 'Clone using the SSH URL'}
		/>
	</div>
	<div class="clone__field repositoryTargetPath">
		<div class="text-base-13 text-semibold clone__field--label">Where to clone</div>
		<TextBox bind:value={targetDirPath} placeholder={'/Users/tipsy/Documents'} />
		<Button
			style="ghost"
			outline
			kind="solid"
			disabled={loading}
			on:click={handleCloneTargetSelect}
		>
			Choose..
		</Button>
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
	<Button style="ghost" outline kind="solid" disabled={loading} on:click={handleCancel}>
		Cancel
	</Button>
	<Button
		style="pop"
		kind="solid"
		icon={errors.length > 0 ? 'update-small' : 'chevron-right-small'}
		disabled={loading}
		{loading}
		on:click={cloneRepository}
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

{#snippet Notification({ title, items, style }: { title: string, items?: any[], style: MessageStyle})}
	<div class="clone__info-message">
		<InfoMessage {style} filled outlined={false}>
			<svelte:fragment slot="title">
				{title}
			</svelte:fragment>
			<svelte:fragment slot="content">
				{#if items && items.length > 0}
					{#each items as item}
						{@html item.label}
					{/each}
				{/if}
			</svelte:fragment>
		</InfoMessage>
	</div>
{/snippet}

<style>
	.clone-title {
		color: var(--clr-scale-ntrl-0);
		line-height: 1;
		margin-bottom: 20px;
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
		gap: 8px;
		justify-content: end;
	}

	.clone__info-message {
		margin-bottom: 20px;
	}
</style>
