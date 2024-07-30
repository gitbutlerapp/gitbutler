<script lang="ts">
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import Section from '$lib/settings/Section.svelte';
	import Button from '$lib/shared/Button.svelte';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';

	const RemoteType = {
		url: 'url',
		ssh: 'ssh'
	} as const;

	let loading = $state(false);
	let errors = $state(0);
	let completed = $state(false);
	let repositoryUrl = $state('');
	let filePath = $state('');
	let remoteType = $state<keyof typeof RemoteType>();

	const remoteTypes = [
		{
			value: 'url',
			label: 'URL'
		},
		{
			value: 'ssh',
			label: 'SSH'
		}
	];

	function cloneRepository() {
		console.log({ repositoryUrl, filePath });
	}
</script>

<DecorativeSplitView>
	<h1 class="clone-title text-serif-40">Clone a repository</h1>
	<Section spacer>
		<div class="clone__remoteType">
			<fieldset name="remoteType" class="remoteType-group">
				{#each remoteTypes as type}
					<label class:selected={type.value === remoteType} for="remoteType-{type.value}">
						<input
							type="radio"
							id="remoteType-{type.value}"
							name="remoteType"
							value={remoteType || RemoteType.url}
							checked={type.value === remoteType}
						/>
						<span class="text-base-12 text-semibold">{type.label}</span>
					</label>
				{/each}
			</fieldset>
		</div>
		<div class="clone__repositoryUrl">
			<TextBox
				bind:value={repositoryUrl}
				required
				on:change={(e) => (repositoryUrl = e.detail)}
				placeholder="https://"
			/>
		</div>
		<div class="clone__repositoryTargetPath">
			<TextBox
				bind:value={filePath}
				required
				on:change={(e) => (filePath = e.detail)}
				placeholder="/Users/tipsy/Documents"
			/>
		</div>
	</Section>
	<div class="clone__actions">
		<Button style="ghost" kind="solid" disabled={loading} on:click={cloneRepository}>Cancel</Button>
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
	}

	.clone__actions {
		display: flex;
		justify-content: end;
	}
</style>
