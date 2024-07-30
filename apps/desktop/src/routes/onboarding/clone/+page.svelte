<script lang="ts">
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import Section from '$lib/settings/Section.svelte';
	import Button from '$lib/shared/Button.svelte';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Segment from '@gitbutler/ui/SegmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/SegmentControl/SegmentControl.svelte';

	const RemoteType = {
		url: 'url',
		ssh: 'ssh'
	} as const;

	let loading = $state(false);
	let errors = $state(0);
	let completed = $state(false);
	let repositoryUrl = $state('');
	let filePath = $state('');
	// TODO: Fix types
	let remoteType = $state<string | keyof typeof RemoteType>(RemoteType.url);

	function cloneRepository() {
		console.log({ repositoryUrl, filePath });
	}
</script>

<DecorativeSplitView>
	<h1 class="clone-title text-serif-40">Clone a repository</h1>
	<Section spacer>
		<div class="clone__remoteType">
			<fieldset name="remoteType" class="remoteType-group">
				<SegmentControl fullWidth selectedIndex={0} onselect={(id) => (remoteType = id)}>
					<Segment id="url">URL</Segment>
					<Segment id="ssh">SSH</Segment>
				</SegmentControl>
			</fieldset>
		</div>
		<div class="clone__field repositoryUrl">
			<TextBox
				bind:value={repositoryUrl}
				required
				on:change={(e) => (repositoryUrl = e.detail)}
				placeholder={remoteType === 'url' ? 'https://..' : 'git@github.com:..'}
			/>
			<div class="text-base-11 clone__field--label">Clone using the web URL</div>
		</div>
		<div class="clone__field repositoryTargetPath">
			<div class="text-base-13 text-semibold clone__field--label">Where to clone</div>
			<TextBox
				bind:value={filePath}
				required
				on:change={(e) => (filePath = e.detail)}
				placeholder={'/Users/tipsy/Documents'}
			/>
			<Button style="ghost" outline kind="solid" disabled={loading}>Choose..</Button>
		</div>
	</Section>
	<div class="clone__actions">
		<Button style="ghost" outline kind="solid" disabled={loading} on:click={cloneRepository}
			>Cancel</Button
		>
		<Button
			style="pop"
			kind="solid"
			icon={errors > 0 ? 'update-small' : 'chevron-right-small'}
			disabled={loading}
			on:click={cloneRepository}
		>
			{#if loading}
				Cloning..
			{:else if errors > 0}
				Retry clone
			{:else}
				Clone
			{/if}
		</Button>
	</div>

	{#if errors || completed}
		<InfoMessage
			style={errors > 0 ? 'warning' : loading ? 'neutral' : 'success'}
			filled
			outlined={false}
		>
			<svelte:fragment slot="title">
				{#if errors > 0}
					There was a problem with your repository
				{:else}
					Clone success
				{/if}
			</svelte:fragment>
			<svelte:fragment slot="content">
				{#if errors > 0}
					TODO: Print error content
				{:else}
					Repository XYZ has been cloned successfully
				{/if}
			</svelte:fragment>
		</InfoMessage>
	{/if}
</DecorativeSplitView>

<style>
	.clone-title {
		color: var(--clr-scale-ntrl-0);
		line-height: 1;
		margin-bottom: 16px;
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
</style>
