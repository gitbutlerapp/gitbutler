<script lang="ts">
	import type { LayoutData } from './$types';
	import { getContext, onMount } from 'svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { Code } from '$lib/backend/ipc';
	import Resizer from '$lib/components/Resizer.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import IconChevronLeft from '$lib/icons/IconChevronLeft.svelte';
	import { goto } from '$app/navigation';
	import BaseBranchSelect from './BaseBranchSelect.svelte';
	import { unsubscribe } from '$lib/utils/random';
	import * as hotkeys from '$lib/utils/hotkeys';
	import { userStore } from '$lib/stores/user';
	import Navigation from './Navigation.svelte';
	import Link from '$lib/components/Link.svelte';
	import Button from '$lib/components/Button.svelte';

	export let data: LayoutData;
	let {
		projectId,
		cloud,
		update,
		sessionsStore,
		deltasStore,
		baseBranchStore,
		baseBranchesState,
		vbranchesState,
		branchController,
		branchesWithContent,
		remoteBranchStore,
		githubContextStore
	} = data;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	$: sessionId = $sessionsStore?.length > 0 ? $sessionsStore[0].id : undefined;
	$: updateDeltasStore(sessionId);

	let trayViewport: HTMLElement;
	let peekTrayExpanded: boolean;

	// Used to prevent peek tray from showing while reducing tray size
	let peekTransitionsDisabled = false;

	// function exists to update the session id as it changes
	function updateDeltasStore(sid: string | undefined) {
		if (sid) deltasStore.setSessionId(sid);
	}

	onMount(() => unsubscribe(hotkeys.on('Meta+Shift+R', () => goto(`/old/${projectId}/player`))));
</script>

{#if $baseBranchesState.isLoading}
	Loading...
{:else if $baseBranchStore}
	{#if !$vbranchesState.isError}
		<div class="relative flex w-full max-w-full" role="group" on:dragover|preventDefault>
			<div bind:this={trayViewport} class="z-30 flex flex-shrink">
				<Navigation
					branchesWithContentStore={branchesWithContent}
					{remoteBranchStore}
					{baseBranchStore}
					{branchController}
					{peekTransitionsDisabled}
					bind:peekTrayExpanded
					{cloud}
					{projectId}
					githubContext={$githubContextStore}
					user={$userStore}
					{update}
				/>
			</div>
			<Resizer
				minWidth={300}
				viewport={trayViewport}
				direction="horizontal"
				class="z-30"
				on:resizing={(e) => {
					peekTransitionsDisabled = e.detail;
				}}
				on:width={(e) => {
					userSettings.update((s) => ({
						...s,
						trayWidth: e.detail
					}));
				}}
			/>
			<slot />
		</div>
	{:else}
		<div class="text-color-3 flex h-full w-full items-center justify-center">
			{#if $vbranchesState.error.code === Code.ProjectHead}
				<div class="flex max-w-xl flex-col justify-center gap-y-3 p-4 text-center">
					<h2 class="text-lg font-semibold">
						Looks like you've switched from gitbutler/integration
					</h2>

					<p>
						Due to GitButler managing multiple virtual branches, you cannot switch back and forth
						between git branches and virtual branches easily.
					</p>

					<Link href="https://docs.gitbutler.com/features/virtual-branches/integration-branch">
						Learn more
					</Link>

					<div class="flex flex-col items-center">
						<Button
							color="purple"
							height="small"
							on:click={() => {
								if ($baseBranchStore) branchController.setTarget($baseBranchStore.branchName);
							}}
						>
							Go back to gitbutler/integration
						</Button>
					</div>
				</div>
			{:else}
				<div class="flex max-w-xl gap-x-2 p-4">
					<IconButton icon={IconChevronLeft} on:click={() => goto('/')}></IconButton>
					<div>
						<h1 class="text-lg font-semibold">There was a problem loading this repo</h1>
						<p>{$vbranchesState.error.message}</p>
					</div>
				</div>
			{/if}
		</div>
	{/if}
{:else}
	<BaseBranchSelect {projectId} {branchController} />
{/if}
