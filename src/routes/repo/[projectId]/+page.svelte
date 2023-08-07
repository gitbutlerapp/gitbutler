<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import { Button } from '$lib/components';
	import { BranchController } from '$lib/vbranches/branchController';
	import { getContext, onDestroy } from 'svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import BottomPanel from './BottomPanel.svelte';
	import UpstreamBranchLane from './UpstreamBranchLane.svelte';
	import { IconExternalLink } from '$lib/icons';
	import {
		getBaseBranchStore,
		getRemoteBranchStore,
		getVirtualBranchStore,
		getWithContentStore
	} from '$lib/vbranches/branchStoresCache';
	import { getSessionStore2 } from '$lib/stores/sessions';
	import { getDeltasStore2 } from '$lib/stores/deltas';
	import { getFetchesStore } from '$lib/stores/fetches';
	// import TopBar from './topbar/TopBar.svelte';

	export let data: PageData;
	let { projectId, remoteBranchNames, project, cloud } = data;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	const fetchStore = getFetchesStore(projectId);
	const deltasStore = getDeltasStore2(projectId);
	const sessionsStore = getSessionStore2(projectId);
	const baseBranchStore = getBaseBranchStore(projectId, [fetchStore]);
	const remoteBranchStore = getRemoteBranchStore(projectId, [fetchStore]);
	const vbranchStore = getVirtualBranchStore(projectId, [deltasStore, sessionsStore]);
	const branchesWithContent = getWithContentStore(projectId, sessionsStore, vbranchStore);

	const fetchUnsubscribe = fetchStore.subscribeStream();
	const sessionsUnsubscribe = sessionsStore.subscribeStream();

	const branchesState = branchesWithContent.state;
	const baseBranchesState = baseBranchStore.state;
	const remoteBranchesState = remoteBranchStore.state;

	const branchController = new BranchController(
		projectId,
		vbranchStore,
		remoteBranchStore,
		baseBranchStore
	);

	$: sessionId = $sessionsStore?.at(-1)?.id;
	$: updateDeltasStore(projectId, sessionId); // has to come before `getVirtualBranchStore`

	let targetChoice: string | undefined;
	let deltasUnsubscribe: () => void;

	// function exists to unsubscribe from delta store when session changes
	function updateDeltasStore(projectId: string, sessionId: string | undefined) {
		if (deltasUnsubscribe) deltasUnsubscribe();
		if (sessionId) {
			deltasUnsubscribe = deltasStore.subscribeStream(sessionId);
		}
	}

	function onSetTargetClick() {
		if (!targetChoice) {
			return;
		}
		branchController.setTarget(targetChoice);
	}

	onDestroy(() => {
		sessionsUnsubscribe();
		fetchUnsubscribe();
		if (deltasUnsubscribe) deltasUnsubscribe();
	});
</script>

{#if $baseBranchStore}
	<div class="flex w-full max-w-full" role="group" on:dragover|preventDefault>
		<Tray
			branches={$vbranchStore}
			branchesState={$branchesState}
			remoteBranches={$remoteBranchStore}
			remoteBranchesState={$remoteBranchesState}
			{branchController}
		/>
		<div
			class="z-30 -ml-[0.250rem] w-[0.250rem] shrink-0 cursor-col-resize hover:bg-orange-200 dark:bg-dark-1000 dark:hover:bg-orange-700"
			draggable="true"
			role="separator"
			on:drag={(e) => {
				userSettings.update((s) => ({
					...s,
					trayWidth: e.clientX
				}));
			}}
		/>
		<div class="flex w-full flex-col overflow-hidden">
			<!-- <TopBar
				vbranches={$vbranchStore}
				{branchController}
				remoteUrl={$baseBranchStore?.remoteUrl}
				remoteBranches={$remoteBranchStore}
			/> -->
			<div
				class="lane-scroll flex flex-grow gap-1 overflow-x-auto overflow-y-hidden overscroll-y-none bg-light-300 dark:bg-dark-1100"
			>
				<UpstreamBranchLane base={$baseBranchStore} {branchController} />
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
			<BottomPanel base={$baseBranchStore} {userSettings} />
		</div>
	</div>
{:else}
	<div class="grid h-full w-full grid-cols-2 items-center justify-items-stretch">
		<div
			id="vb-data"
			class="flex h-full flex-col justify-center gap-y-4 self-center bg-light-400 p-12 text-lg dark:bg-dark-700"
		>
			<div class="font-bold">Set your Base Branch</div>
			<p class="text-light-700 dark:text-dark-100">
				You need to set your base branch before you can start working on your project.
			</p>
			<!-- select menu of remoteBranches -->
			{#if remoteBranchNames.length === 0}
				<p class="mt-6 text-red-500">You don't have any remote branches.</p>
				<p class="mt-6 text-sm text-light-700">
					Currently, GitButler requires a remote branch to base it's virtual branch work on. To use
					virtual branches, please push your code to a remote branch to use as a base.
					<a
						target="_blank"
						rel="noreferrer"
						class="font-bold"
						href="https://docs.gitbutler.com/features/virtual-branches/butler-flow">Learn more</a
					>
				</p>
			{:else}
				<select bind:value={targetChoice}>
					{#each remoteBranchNames
						.map((branch) => branch.substring(13))
						.sort((a, b) => a.localeCompare(b)) as branch}
						{#if branch == 'origin/master' || branch == 'origin/main'}
							<option value={branch} selected>{branch}</option>
						{:else}
							<option value={branch}>{branch}</option>
						{/if}
					{/each}
				</select>
				<p class="text-base text-light-700 dark:text-dark-100">
					This is the branch that you consider "production", normally something like "origin/master"
					or "origin/main".
				</p>
				<div>
					<Button color="purple" height="small" on:click={onSetTargetClick}>Set Base Branch</Button>
				</div>
			{/if}
		</div>
		<div id="vb-data" class="max-h-full justify-center overflow-y-auto">
			<div class="flex h-full max-h-full flex-col gap-y-3 p-12 text-lg">
				<h1 class="text-xl font-bold">Getting Started with Virtual Branches</h1>
				<p class="text-xl text-light-700 dark:text-dark-100">
					Virtual branches are just like normal Git branches, except that you can work on several of
					them at the same time.
				</p>
				<div class="font-bold">Base Branch</div>
				<p class="text-light-700 dark:text-dark-100">
					With virtual branches, you are not working off of local main or master branches.
					Everything that you do is on a virtual branch, automatically.
				</p>
				<p class="text-light-700 dark:text-dark-100">
					This works by specifying a "base branch" that represents the state of production, normally
					something like "origin/master".
				</p>
				<div class="font-bold">Ownership, Committing and Pushing</div>
				<p class="text-light-700 dark:text-dark-100">
					Each virtual branch "owns" parts of the files that are seen as changed. If you commit on
					that branch, only the parts that are owned by that branch are actually recorded in the
					commits on that branch.
				</p>
				<p class="text-light-700 dark:text-dark-100">
					When you push a virtual branch, it will create a branch name based on your branch title,
					push that branch to your remote with just the changes committed to that branch, not
					everything in your working directory.
				</p>
				<div class="font-bold">Applying and Unapplying</div>
				<p class="text-light-700 dark:text-dark-100">
					You can have many virtual branches applied at the same time, but they cannot conflict with
					each other currently. Unapplying a virtual branch will take all of the changes that it
					owns and remove them from your working directory. Applying the branch will add those
					changes back in.
				</p>
				<div class="flex flex-row place-content-center content-center space-x-2 pt-4 text-blue-600">
					<a
						target="_blank"
						rel="noreferrer"
						class="font-bold"
						href="https://docs.gitbutler.com/features/virtual-branches">Learn more</a
					>
					<IconExternalLink class="h-4 w-4" />
				</div>
			</div>
		</div>
	</div>
{/if}
