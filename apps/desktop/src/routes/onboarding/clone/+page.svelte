<script lang="ts">
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import Section from '$lib/settings/Section.svelte';
	import Button from '$lib/shared/Button.svelte';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Segment from '@gitbutler/ui/SegmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/SegmentControl/SegmentControl.svelte';
	import { open } from '@tauri-apps/api/dialog';
	import { goto } from '$app/navigation';

	const RemoteType = {
		url: 'url',
		ssh: 'ssh'
	} as const;

	let loading = $state(false);
	let errors = $state<{ label: string }[]>([]);
	let completed = $state(false);
	let repositoryUrl = $state('');
	let filePath = $state('');
	// TODO: Fix types
	let remoteType = $state<string | keyof typeof RemoteType>(RemoteType.url);

	async function handleCloneTargetSelect() {
		const selectedPath = await open({ directory: true, recursive: true });
		if (!selectedPath) return;

		filePath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;
	}

	function cloneRepository() {
		if (errors.length) {
			errors = [];
		}
		console.log({ repositoryUrl, filePath });

		if (!repositoryUrl || !filePath) {
			errors.push({
				label: 'You must add both a repository URL and target file path.'
			});
		}
	}

	function handleCancel() {
		goto('/onboarding');
	}
</script>

<DecorativeSplitView>
	<h1 class="clone-title text-serif-40">Clone a repository</h1>
	<Section>
		<div class="clone__remoteType">
			<fieldset name="remoteType" class="remoteType-group">
				<SegmentControl fullWidth defaultIndex={0} onselect={(id) => (remoteType = id)}>
					<Segment id="url">URL</Segment>
					<Segment id="ssh">SSH</Segment>
				</SegmentControl>
			</fieldset>
		</div>
		<div class="clone__field repositoryUrl">
			<TextBox
				bind:value={repositoryUrl}
				placeholder={remoteType === 'url' ? 'https://..' : 'git@github.com:..'}
			/>
			<div class="text-base-11 clone__field--label">Clone using the web URL</div>
		</div>
		<div class="clone__field repositoryTargetPath">
			<div class="text-base-13 text-semibold clone__field--label">Where to clone</div>
			<TextBox bind:value={filePath} placeholder={'/Users/tipsy/Documents'} />
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

	{#if errors.length || completed}
		<div class="clone__info-message">
			<InfoMessage
				style={errors.length > 0 ? 'warning' : loading ? 'neutral' : 'success'}
				filled
				outlined={false}
			>
				<svelte:fragment slot="title">
					{#if errors.length > 0}
						There was a problem cloning your repository
					{:else}
						Clone success
					{/if}
				</svelte:fragment>
				<svelte:fragment slot="content">
					{#if errors.length > 0}
						{#each errors as error}
							{error.label}
						{/each}
					{:else}
						Repository XYZ has been cloned successfully
					{/if}
				</svelte:fragment>
			</InfoMessage>
		</div>
	{/if}

	<Spacer />
	<div class="clone__actions">
		<Button style="ghost" outline kind="solid" disabled={loading} on:click={handleCancel}>
			Cancel
		</Button>
		<Button
			style="pop"
			kind="solid"
			icon={errors.length > 0 ? 'update-small' : 'chevron-right-small'}
			disabled={loading}
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
</DecorativeSplitView>

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
