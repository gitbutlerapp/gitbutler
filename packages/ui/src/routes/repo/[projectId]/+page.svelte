<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import { Button, Link } from '$lib/components';
	import { BranchController } from '$lib/vbranches/branchController';
	import { getContext } from 'svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import { IconExternalLink } from '$lib/icons';
	import {
		getBaseBranchStore,
		getRemoteBranchStore,
		getVirtualBranchStore,
		getWithContentStore
	} from '$lib/vbranches/branchStoresCache';
	import { getHeadsStore } from '$lib/stores/head';
	import { getSessionStore } from '$lib/stores/sessions';
	import { getDeltasStore } from '$lib/stores/deltas';
	import { getFetchesStore } from '$lib/stores/fetches';
	import { Code } from '$lib/ipc';
	import Resizer from '$lib/components/Resizer.svelte';
	import { projectHttpsWarningBannerDismissed } from '$lib/config/config';
	import IconButton from '$lib/components/IconButton.svelte';
	import IconChevronLeft from '$lib/icons/IconChevronLeft.svelte';
	import { goto } from '$app/navigation';
	import BaseBranchSelect from './BaseBranchSelect.svelte';

	export let data: PageData;
	let { projectId, project, cloud } = data;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	const fetchStore = getFetchesStore(projectId);
	const deltasStore = getDeltasStore(projectId);
	const headStore = getHeadsStore(projectId);
	const sessionsStore = getSessionStore(projectId);
	const baseBranchStore = getBaseBranchStore(projectId, fetchStore, headStore);
	const remoteBranchStore = getRemoteBranchStore(projectId, fetchStore, headStore, baseBranchStore);
	const vbranchStore = getVirtualBranchStore(
		projectId,
		deltasStore,
		sessionsStore,
		headStore,
		baseBranchStore
	);
	const branchesWithContent = getWithContentStore(projectId, sessionsStore, vbranchStore);

	const vbrachesState = vbranchStore.state;
	const branchesState = branchesWithContent.state;
	const baseBranchesState = baseBranchStore.state;

	const branchController = new BranchController(
		projectId,
		vbranchStore,
		remoteBranchStore,
		baseBranchStore
	);

	const httpsWarningBannerDismissed = projectHttpsWarningBannerDismissed(projectId);

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
</script>

{#if $baseBranchesState.isLoading}
	Loading...
{:else if $baseBranchStore}
	{#if !$vbrachesState.isError}
		<div class="relative flex w-full max-w-full" role="group" on:dragover|preventDefault>
			<div bind:this={trayViewport} class="z-30 flex flex-shrink">
				<Tray
					branchesWithContentStore={branchesWithContent}
					{remoteBranchStore}
					{baseBranchStore}
					{branchController}
					{peekTransitionsDisabled}
					bind:peekTrayExpanded
					{cloud}
					{projectId}
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
			<div class="flex w-full flex-col overflow-hidden">
				{#if $baseBranchStore?.remoteUrl.startsWith('https') && !$httpsWarningBannerDismissed}
					<div class="flex items-center bg-yellow-200/70 px-2 py-1 dark:bg-yellow-700/70">
						<div class="flex flex-grow">
							HTTPS remote detected. In order to push & fetch, you may need to&nbsp;
							<a class="font-bold" href="/user"> set up </a>&nbsp;an SSH key (
							<a
								target="_blank"
								rel="noreferrer"
								class="font-bold"
								href="https://docs.gitbutler.com/features/virtual-branches/pushing-and-fetching#the-ssh-keys"
							>
								docs
							</a>
							&nbsp;
							<IconExternalLink class="h-4 w-4" />
							).
						</div>

						<button on:click={() => httpsWarningBannerDismissed.set(true)}>Dismiss</button>
					</div>
				{/if}
				<div
					class="lane-scroll flex flex-grow gap-1 overflow-x-auto overflow-y-hidden overscroll-none transition-opacity duration-300"
					style:opacity={peekTrayExpanded ? '0.5' : undefined}
				>
					<Board
						branches={$branchesWithContent}
						branchesState={$branchesState}
						{branchController}
						{projectId}
						projectPath={$project?.path}
						base={$baseBranchStore}
						baseBranchState={$baseBranchesState}
						cloudEnabled={$project?.api?.sync || false}
						{cloud}
					/>
				</div>
				<!-- <BottomPanel base={$baseBranchStore} {userSettings} /> -->
			</div>
		</div>
	{:else}
		<div class="text-color-3 flex h-full w-full items-center justify-center">
			{#if $vbrachesState.error.code === Code.ProjectHead}
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
						<p>{$vbrachesState.error.message}</p>
					</div>
				</div>
			{/if}
		</div>
	{/if}
{:else}
	<BaseBranchSelect {projectId} {branchController} />
{/if}
