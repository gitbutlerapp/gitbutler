<script async lang="ts">
	import Button from '$lib/components/Button.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { goto } from '$app/navigation';
	import CardSection from './components/CardSection.svelte';
	import Login from '$lib/components/Login.svelte';
	import type { UserService } from '$lib/stores/user';
	import { getContext } from 'svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import GithubIntegration from '../components/GithubIntegration.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import { getRemoteBranches } from '$lib/vbranches/branchStoresCache';
	import { projectAiGenEnabled } from '$lib/config/config';

	export let branchController: BranchController;
	export let userService: UserService;
	export let projectId: string;

	$: user$ = userService.user$;

	const remoteBranches = getRemoteBranches(projectId);
	const aiGenEnabled = projectAiGenEnabled(projectId);

	let aiGenCheckbox: HTMLInputElement;
	let targetChoice: string | undefined;
	let loading = false;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	function onSetTargetClick() {
		if (!targetChoice) {
			return;
		}
		loading = true;
		branchController.setTarget(targetChoice).finally(() => (loading = false));
	}
</script>

<div class="wrapper">
	<div class="setup">
		<div class="setup__header">
			<span class="setup__title text-base-14 font-semibold">Setup</span>
		</div>
		<div class="setup__content">
			{#await remoteBranches}
				<p>loading...</p>
			{:then names}
				{#if names.length == 0}
					<p class="mt-6 text-red-500">You don't have any remote branches.</p>
					<p class="text-color-3 mt-6 text-sm">
						Currently, GitButler requires a remote branch to base it's virtual branch work on. To
						use virtual branches, please push your code to a remote branch to use as a base.
						<a
							target="_blank"
							rel="noreferrer"
							class="font-bold"
							href="https://docs.gitbutler.com/features/virtual-branches/butler-flow">Learn more</a
						>
					</p>
				{:else}
					<CardSection>
						<svelte:fragment slot="label">Choose your base branch</svelte:fragment>
						<svelte:fragment slot="description">
							This is the branch that you consider "production", normally something like
							"origin/master" or "origin/main".
						</svelte:fragment>
						<select class="select" bind:value={targetChoice} disabled={loading}>
							{#each names
								.map((name) => name.substring(13))
								.sort((a, b) => a.localeCompare(b)) as branch}
								{#if branch == 'origin/master' || branch == 'origin/main'}
									<option value={branch} selected>{branch}</option>
								{:else}
									<option value={branch}>{branch}</option>
								{/if}
							{/each}
						</select>
					</CardSection>
					<CardSection>
						<svelte:fragment slot="label">
							Take advantage of our AI integration <span class="optional">(optional)</span>
						</svelte:fragment>
						<svelte:fragment slot="description">
							Enable automatic branch and commit message generation by logging or setting up an
							account.
						</svelte:fragment>
						{#if !$userSettings.aiSummariesEnabled}
							{#if !$user$}
								<Login {userService} />
							{:else}
								<input
									id="summarize"
									bind:this={aiGenCheckbox}
									type="checkbox"
									checked={$aiGenEnabled}
									on:change={() => {
										$aiGenEnabled = aiGenCheckbox.checked;
									}}
								/>
								<label for="summarize">Enable automatic summaries of your work</label>
							{/if}
						{/if}
					</CardSection>
					<CardSection disabled={!$user$}>
						<svelte:fragment slot="label">
							Work seamlessly with GitHub pull requests <span class="optional">
								{#if $user$}
									(optional)
								{:else}
									(requires login)
								{/if}
							</span>
						</svelte:fragment>
						<svelte:fragment slot="description">
							Enables you to work with PRs without leaving the app.
						</svelte:fragment>
						{#if !$user$?.github_access_token}
							<GithubIntegration minimal {userService} />
						{:else}
							<Icon name="tick" color="success" /> You have enabled this integration
						{/if}
					</CardSection>
				{/if}
			{:catch}
				<p>Something has gone wrong...</p>
			{/await}
		</div>

		<div class="setup_footer">
			<IconButton icon="home" on:click={() => goto('/')} />
			<Button color="primary" {loading} on:click={onSetTargetClick} id="set-base-branch">
				Done
			</Button>
		</div>
	</div>
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		justify-content: center;
		align-items: center;
		width: 100%;
		padding: var(--space-24);
	}
	.setup {
		display: flex;
		flex-direction: column;
		max-width: 640px;
		overflow-y: hidden;
		background: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		border-radius: var(--radius-m);
	}
	.setup__header {
		padding: var(--space-12);
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
	}
	.setup__title {
		padding: var(--space-4) var(--space-6);
	}
	.setup__content {
		display: flex;
		flex-direction: column;
		gap: var(--space-28);
		padding: var(--space-24) var(--space-16);
	}
	.setup_footer {
		display: flex;
		gap: var(--space-6);
		padding: var(--space-12);
		justify-content: space-between;
		border-top: 1px solid var(--clr-theme-container-outline-light);
	}

	.optional {
		color: var(--clr-theme-scale-ntrl-60);
		font-style: italic;
		font-weight: 500;
	}
	.select {
		width: 100%;
	}
</style>
