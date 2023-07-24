<script lang="ts">
	import Board from './Board.svelte';
	import Tray from './Tray.svelte';
	import type { PageData } from './$types';
	import { Button } from '$lib/components';
	import { BranchController } from '$lib/vbranches';
	import { Loaded } from 'svelte-loadable-store';
	import { getContext, setContext } from 'svelte';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import BottomPanel from './BottomPanel.svelte';
	import UpstreamBranchLane from './UpstreamBranchLane.svelte';
	import { IconExternalLink } from '$lib/icons';

	export let data: PageData;
	let {
		projectId,
		vbranchStore,
		remoteBranchStore,
		targetBranchStore,
		remoteBranchNames,
		project,
		cloud
	} = data;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	const branchController = new BranchController(
		projectId,
		vbranchStore,
		remoteBranchStore,
		targetBranchStore
	);
	setContext(BRANCH_CONTROLLER_KEY, branchController);

	$: remoteBranches = Loaded.isValue($remoteBranchStore) ? $remoteBranchStore.value : [];
	$: base = Loaded.isValue($targetBranchStore) ? $targetBranchStore.value : undefined;
	$: branches =
		!$vbranchStore.isLoading && !Loaded.isError($vbranchStore) ? $vbranchStore.value : [];
	let targetChoice: string | undefined;

	function onSetTargetClick() {
		if (!targetChoice) {
			return;
		}
		branchController.setTarget(targetChoice);
	}
</script>

{#if base}
	<div class="flex w-full max-w-full" role="group" on:dragover|preventDefault>
		<Tray {branches} {remoteBranches} />
		<div
			class="z-50 -ml-[0.250rem] w-[0.250rem] shrink-0 cursor-col-resize hover:bg-orange-200 dark:bg-dark-1000 dark:hover:bg-orange-700"
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
			<div class="lane-scroll flex flex-grow overflow-x-auto overflow-y-hidden overscroll-y-none">
				{#if base}
					<UpstreamBranchLane {base} />
				{/if}
				<Board
					{branches}
					{projectId}
					projectPath={$project?.path}
					{base}
					cloudEnabled={$project?.api?.sync || false}
					{cloud}
				/>
			</div>
			<BottomPanel {base} {userSettings} />
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
						href="https://docs.gitbutler.com/features/virtual-branches/base-branch">Learn more</a
					>
				</p>
			{:else}
				<select bind:value={targetChoice}>
					{#each remoteBranchNames as branch}
						{@const strippedBranch = branch.substring(13)}
						{#if strippedBranch == 'origin/master' || strippedBranch == 'origin/main'}
							<option value={strippedBranch} selected>{strippedBranch}</option>
						{:else}
							<option value={strippedBranch}>{strippedBranch}</option>
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
		<div
			id="vb-data"
			class="flex h-full flex-col justify-center gap-y-3 overflow-y-auto p-12 text-lg"
		>
			<h1 class="text-xl font-bold">Getting Started with Virtual Branches</h1>
			<p class="text-xl text-light-700 dark:text-dark-100">
				Virtual branches are just like normal Git branches, except that you can work on several of
				them at the same time.
			</p>
			<div class="font-bold">Base Branch</div>
			<p class="text-light-700 dark:text-dark-100">
				With virtual branches, you are not working off of local main or master branches. Everything
				that you do is on a virtual branch, automatically.
			</p>
			<p class="text-light-700 dark:text-dark-100">
				This works by specifying a "base branch" that represents the state of production, normally
				something like "origin/master". All of your virtual branches are based off of this branch
				and need to be kept up to date with this branch to ensure they are working with the latest
				code.
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
				each other currently. Unapplying a virtual branch will take all of the changes that it owns
				and remove them from your working directory. Applying the branch will add those changes back
				in.
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
{/if}
