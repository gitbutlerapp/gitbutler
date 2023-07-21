<script lang="ts">
	import { IconBranch, IconRefresh } from '$lib/icons';
	import { Button, Modal } from '$lib/components';
	import type { BaseBranch } from '$lib/vbranches';
	import CommitCard from './CommitCard.svelte';
	import type { BranchController } from '$lib/vbranches';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';
	import { getContext } from 'svelte';

	export let baseBranch: BaseBranch;

	let updateTargetModal: Modal;

	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);

	$: behind = baseBranch.behind > 0;
	$: behindMessage = baseBranch.behind > 0 ? `behind ${baseBranch.behind}` : 'up-to-date';
</script>

<div
	class:w-64={behind}
	class:w-16={!behind}
	class="flex h-full shrink-0 cursor-default snap-center flex-col overflow-y-scroll overscroll-y-none border-r border-r-blue-200 bg-light-200 pt-2 transition-width dark:border-r-light-800 dark:bg-dark-1000 dark:text-dark-100"
	role="group"
>
	{#if behind}
		<div class="p-2">
			<div class="flex flex-row justify-between px-2 pb-2">
				<div class="flex flex-row space-x-2">
					<IconBranch class="h-4 w-4" />
					<div
						class="w-full truncate border-0 font-bold text-light-900 dark:bg-dark-1000 dark:text-dark-100"
					>
						{baseBranch.branchName}
					</div>
				</div>
				<div class="text-sm text-light-600">
					{behindMessage}
				</div>
			</div>
			<Button class="w-full" height="small" color="purple" on:click={updateTargetModal.show}>
				Merge Upstream
			</Button>
			<div class="flex h-full">
				<div class="relative z-30 h-full">
					<div
						class="absolute top-0 z-30 ml-[20px] h-full w-px
          bg-gradient-to-b from-transparent via-light-400 dark:via-dark-600
          "
					/>
				</div>
				<div class="z-40 mt-4 flex w-full flex-col gap-2">
					{#each baseBranch.upstreamCommits as commit}
						<CommitCard {commit} />
					{/each}
				</div>
			</div>
		</div>
	{:else}
		<div class="p-2">
			<div class="text-light-600">Up to date</div>
			<button
				class="p-0 text-light-600 hover:bg-light-200 disabled:text-light-300 dark:hover:bg-dark-800 disabled:dark:text-dark-300"
				on:click={() => branchController.fetchFromTarget()}
				title="click to fetch"
			>
				<IconRefresh />
			</button>
		</div>
	{/if}
</div>

<!-- Confirm target update modal -->

<Modal width="small" bind:this={updateTargetModal}>
	<svelte:fragment slot="title">Update target</svelte:fragment>
	<p>You are about to update the base branch.</p>
	<svelte:fragment slot="controls" let:close>
		<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
		<Button
			height="small"
			color="purple"
			on:click={() => {
				branchController.updateBaseBranch();
				close();
			}}
		>
			Update
		</Button>
	</svelte:fragment>
</Modal>
