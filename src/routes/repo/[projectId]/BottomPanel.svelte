<script lang="ts">
	import { Button, Modal } from '$lib/components';
	import { slide } from 'svelte/transition';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';
	import type { BaseBranch } from '$lib/vbranches';
	import type { BranchController } from '$lib/vbranches';
	import { IconRefresh } from '$lib/icons';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';
	import { getContext } from 'svelte';

	export let target: BaseBranch;

	let updateTargetModal: Modal;

	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);

	$: console.log(target);
	let shown = false;
	$: behindMessage = target.behind > 0 ? `behind ${target.behind}` : 'up-to-date';
</script>

<div class="flex border-t border-light-400 p-2 dark:border-dark-600">
	<div class="ml-4 flex flex-col">
		<div
			role="button"
			tabindex="0"
			class="flex h-[20px] items-center gap-2 text-light-700 hover:text-light-900 dark:text-dark-200 dark:hover:text-dark-100"
			on:click={() => (shown = !shown)}
			on:keypress={() => (shown = !shown)}
		>
			{#if shown}
				<IconTriangleDown />
			{:else}
				<IconTriangleUp />
			{/if}
			<div class="flex flex-row justify-between space-x-2">
				<div class="font-bold uppercase">Common base</div>
				<div class="flex-grow pb-1 font-bold" title={behindMessage}>{target.branchName}</div>
				<div class="pb-1">{target.behind > 0 ? `behind ${target.behind}` : 'up-to-date'}</div>
				<div class="flex-shrink-0 text-light-700 dark:text-dark-100" title={behindMessage}>
					{#if target.behind == 0}
						<button
							class="p-0 hover:bg-light-200 disabled:text-light-300 dark:hover:bg-dark-800 disabled:dark:text-dark-300"
							on:click={() => branchController.fetchFromTarget()}
							title="click to fetch"
						>
							<IconRefresh />
						</button>
					{:else}
						<button
							class="p-0 disabled:text-light-300 disabled:dark:text-dark-300"
							on:click={updateTargetModal.show}
							disabled={target.behind == 0}
							title={target.behind > 0 ? 'click to update target' : 'already up-to-date'}
						>
							<IconRefresh />
						</button>
					{/if}
				</div>
			</div>
		</div>
		{#if shown}
			<div class="h-64 py-2" transition:slide={{ duration: 150 }}>
				<div>Upstream Commits:</div>
				<div class="flex flex-col">
					{#each target.upstreamCommits as commit}
						<div class="flex flex-row space-x-2">
							<img
								class="relative z-30 inline-block h-4 w-4 rounded-full ring-1 ring-white dark:ring-black"
								title="Gravatar for {commit.author.email}"
								alt="Gravatar for {commit.author.email}"
								srcset="{commit.author.gravatarUrl} 2x"
								width="100"
								height="100"
								on:error
							/>
							<div>{commit.description.substring(0, 50)}</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}
	</div>
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
