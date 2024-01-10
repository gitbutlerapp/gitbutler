<script async lang="ts">
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { goto } from '$app/navigation';
	import Login from '$lib/components/Login.svelte';
	import type { UserService } from '$lib/stores/user';
	import GithubIntegration from '../components/GithubIntegration.svelte';
	import { getRemoteBranches } from '$lib/vbranches/branchStoresCache';
	import { projectAiGenEnabled } from '$lib/config/config';
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import SetupFeature from './components/SetupFeature.svelte';
	import Toggle from '$lib/components/Toggle.svelte';

	export let branchController: BranchController;
	export let userService: UserService;
	export let projectId: string;

	$: user$ = userService.user$;

	const remoteBranches = getRemoteBranches(projectId);
	const aiGenEnabled = projectAiGenEnabled(projectId);

	let aiGenCheckbox: Toggle;
	let targetChoice: string | undefined;
	let loading = false;

	function onSetTargetClick() {
		if (!targetChoice) {
			return;
		}
		loading = true;
		branchController.setTarget(targetChoice).finally(() => (loading = false));
	}
</script>

<DecorativeSplitView user={$user$}>
	<div class="project-setup">
		{#await remoteBranches}
			<p>loading...</p>
		{:then names}
			{#if names.length == 0}
				<p class="mt-6 text-red-500">You don't have any remote branches.</p>
				<p class="text-color-3 mt-6 text-sm">
					Currently, GitButler requires a remote branch to base it's virtual branch work on. To use
					virtual branches, please push your code to a remote branch to use as a base.
					<a
						target="_blank"
						rel="noreferrer"
						class="font-bold"
						href="https://docs.gitbutler.com/features/virtual-branches/butler-flow"
					>
						Learn more
					</a>
				</p>
			{:else}
				<div class="project-setup__info">
					<p class="text-base-body-14 text-bold">Target branch</p>
					<p class="text-base-body-12">
						This is the branch that you consider "production", normally something like
						"origin/master" or "origin/main".
					</p>
				</div>
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

				<div class="card">
					<SetupFeature>
						<svelte:fragment slot="icon">
							<svg
								width="20"
								height="20"
								viewBox="0 0 20 20"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path
									fill-rule="evenodd"
									clip-rule="evenodd"
									d="M0 9.84C0 6.25297 0 4.45946 0.74216 3.10948C1.29067 2.11174 2.11174 1.29067 3.10948 0.74216C4.45946 0 6.25297 0 9.84 0H10.16C13.747 0 15.5405 0 16.8905 0.74216C17.8883 1.29067 18.7093 2.11174 19.2578 3.10948C20 4.45946 20 6.25297 20 9.84V10.16C20 13.747 20 15.5405 19.2578 16.8905C18.7093 17.8883 17.8883 18.7093 16.8905 19.2578C15.5405 20 13.747 20 10.16 20H9.84C6.25297 20 4.45946 20 3.10948 19.2578C2.11174 18.7093 1.29067 17.8883 0.74216 16.8905C0 15.5405 0 13.747 0 10.16V9.84ZM3.18635 11.7585C2.93788 11.6757 2.93788 11.3243 3.18635 11.2415L5.68497 10.4086C6.49875 10.1373 7.13732 9.49875 7.40858 8.68497L8.24146 6.18635C8.32428 5.93788 8.67572 5.93788 8.75854 6.18635L9.59142 8.68497C9.86268 9.49875 10.5012 10.1373 11.315 10.4086L13.8137 11.2415C14.0621 11.3243 14.0621 11.6757 13.8137 11.7585L11.315 12.5914C10.5012 12.8627 9.86268 13.5012 9.59142 14.315L8.75854 16.8137C8.67572 17.0621 8.32428 17.0621 8.24146 16.8137L7.40858 14.315C7.13732 13.5012 6.49875 12.8627 5.68497 12.5914L3.18635 11.7585ZM11.1016 5.85898C10.9661 5.90415 10.9661 6.09585 11.1016 6.14102L12.4645 6.59532C12.9084 6.74328 13.2567 7.09159 13.4047 7.53547L13.859 8.89836C13.9042 9.03388 14.0958 9.03388 14.141 8.89836L14.5953 7.53547C14.7433 7.09159 15.0916 6.74328 15.5355 6.59532L16.8984 6.14102C17.0339 6.09585 17.0339 5.90415 16.8984 5.85898L15.5355 5.40468C15.0916 5.25672 14.7433 4.90841 14.5953 4.46453L14.141 3.10164C14.0958 2.96612 13.9042 2.96612 13.859 3.10164L13.4047 4.46453C13.2567 4.90841 12.9084 5.25672 12.4645 5.40468L11.1016 5.85898Z"
									fill="url(#paint0_linear_484_17972)"
								/>
								<defs>
									<linearGradient
										id="paint0_linear_484_17972"
										x1="3.5"
										y1="4"
										x2="16"
										y2="15.5"
										gradientUnits="userSpaceOnUse"
									>
										<stop stop-color="#8E48FF" />
										<stop offset="1" stop-color="#FA7269" />
									</linearGradient>
								</defs>
							</svg>
						</svelte:fragment>
						<svelte:fragment slot="title">GitButler features</svelte:fragment>
						<svelte:fragment slot="body">
							Enable automatic branch and commit message generation.
						</svelte:fragment>
						<svelte:fragment slot="toggle">
							{#if $user$}
								<Toggle
									bind:this={aiGenCheckbox}
									checked={$aiGenEnabled}
									on:change={() => {
										$aiGenEnabled = !$aiGenEnabled;
									}}
								/>
							{/if}
						</svelte:fragment>
						<svelte:fragment slot="actions">
							{#if !$user$}
								<Login {userService} />
							{/if}
						</svelte:fragment>
					</SetupFeature>

					<SetupFeature
						disabled={!$user$}
						success={!!$user$?.github_access_token}
						topBorder={!!$user$ && !$user$?.github_access_token}
					>
						<svelte:fragment slot="icon">
							<svg
								width="20"
								height="20"
								viewBox="0 0 20 20"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path
									fill-rule="evenodd"
									clip-rule="evenodd"
									d="M10.0083 0C4.47396 0 0 4.58331 0 10.2535C0 14.786 2.86662 18.6226 6.84338 19.9805C7.34058 20.0826 7.5227 19.7599 7.5227 19.4885C7.5227 19.2508 7.50631 18.436 7.50631 17.587C4.72225 18.1983 4.14249 16.3647 4.14249 16.3647C3.69508 15.1764 3.03215 14.871 3.03215 14.871C2.12092 14.2429 3.09852 14.2429 3.09852 14.2429C4.1093 14.3108 4.63969 15.2954 4.63969 15.2954C5.53432 16.857 6.97592 16.4158 7.55588 16.1441C7.63865 15.482 7.90394 15.0237 8.18563 14.7691C5.96514 14.5314 3.62891 13.6487 3.62891 9.71017C3.62891 8.58976 4.02634 7.67309 4.65608 6.96018C4.55672 6.7056 4.20866 5.65289 4.75564 4.24394C4.75564 4.24394 5.60069 3.97228 7.5061 5.29644C8.32188 5.07199 9.16317 4.95782 10.0083 4.95685C10.8533 4.95685 11.7148 5.07581 12.5102 5.29644C14.4159 3.97228 15.2609 4.24394 15.2609 4.24394C15.8079 5.65289 15.4596 6.7056 15.3603 6.96018C16.0066 7.67309 16.3876 8.58976 16.3876 9.71017C16.3876 13.6487 14.0514 14.5143 11.8143 14.7691C12.179 15.0916 12.4936 15.7026 12.4936 16.6703C12.4936 18.0453 12.4773 19.1489 12.4773 19.4883C12.4773 19.7599 12.6596 20.0826 13.1566 19.9808C17.1333 18.6224 20 14.786 20 10.2535C20.0163 4.58331 15.526 0 10.0083 0Z"
									fill="black"
								/>
							</svg>
						</svelte:fragment>
						<svelte:fragment slot="title">
							GitHub features
							{#if $user$?.github_access_token}
								enabled
								<svg
									class="inline"
									width="13"
									height="13"
									viewBox="0 0 13 13"
									fill="none"
									xmlns="http://www.w3.org/2000/svg"
								>
									<path
										fill-rule="evenodd"
										clip-rule="evenodd"
										d="M6.5 0C2.91015 0 0 2.91015 0 6.5C0 10.0899 2.91015 13 6.5 13C10.0899 13 13 10.0899 13 6.5C13 2.91015 10.0899 0 6.5 0ZM6.11541 7.12437C6.02319 7.22683 5.86252 7.22683 5.77031 7.12437L4.23194 5.41507L3.19663 6.34684L4.735 8.05614C5.38052 8.77338 6.50519 8.77338 7.15071 8.05614L9.80336 5.10874L8.76806 4.17697L6.11541 7.12437Z"
										fill="#30BB78"
									/>
								</svg>
							{/if}
						</svelte:fragment>
						<svelte:fragment slot="body">
							Enable creation of pull requests from within the app.
						</svelte:fragment>
						<svelte:fragment slot="actions">
							{#if !$user$?.github_access_token}
								<GithubIntegration minimal {userService} />
							{/if}
						</svelte:fragment>
					</SetupFeature>
				</div>
			{/if}
		{:catch}
			<p>Something has gone wrong...</p>
		{/await}
	</div>
	<div class="floating-buttons">
		<Button kind="outlined" on:click={() => goto('/')}>Back</Button>
		<Button {loading} on:click={onSetTargetClick} id="set-base-branch">Let's go!</Button>
	</div>
</DecorativeSplitView>

<style lang="postcss">
	.project-setup {
		max-width: 30rem;
		display: flex;
		flex-direction: column;
		gap: var(--space-20);
	}

	.project-setup__info {
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
	}

	.floating-buttons {
		display: flex;
		position: absolute;
		right: var(--space-32);
		bottom: var(--space-32);
		gap: var(--space-8);
	}

	.select {
		width: 100%;
	}
</style>
