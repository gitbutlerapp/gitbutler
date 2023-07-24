<script lang="ts">
	import { IconBranch, IconRefresh, IconGithub } from '$lib/icons';
	import { Button, Modal, Tooltip } from '$lib/components';
	import type { BaseBranch } from '$lib/vbranches';
	import CommitCard from './CommitCard.svelte';
	import type { BranchController } from '$lib/vbranches';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';
	import { getContext } from 'svelte';

	export let base: BaseBranch;

	let updateTargetModal: Modal;

	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);

	// $: behind = baseBranch.behind > 0;
	$: behindMessage = base.behind > 0 ? `behind ${base.behind}` : 'up-to-date';

	let fetching = false;
	$: expanded = base.behind > 0;
	let buttonHovered = false;
</script>

<div
	class="flex h-full shrink-0 cursor-default snap-center flex-col overflow-y-scroll
	overscroll-y-none border-light-600
	bg-light-400
	dark:border-l
	dark:border-dark-600 dark:border-r-light-800 dark:bg-dark-700 dark:text-dark-100"
	role="group"
>
	<div class="flex w-full items-center gap-4 border-b border-light-500 px-2 dark:border-dark-500">
		<Tooltip label={'Fetch ' + base.branchName}>
			<!-- svelte-ignore a11y-mouse-events-have-key-events -->
			<button
				on:mouseover={() => (buttonHovered = true)}
				on:mouseleave={() => (buttonHovered = false)}
				on:click={() => {
					fetching = true;
					branchController.fetchFromTarget().finally(() => (fetching = false));
				}}
			>
				<div
					class="flex h-6 w-6 items-center justify-center rounded hover:bg-light-200 dark:hover:bg-dark-700"
				>
					{#if buttonHovered || fetching}
						<div class:animate-spin={fetching}>
							<IconRefresh class="h-4 w-4" />
						</div>
					{:else if base.remoteUrl.includes('github.com')}
						<IconGithub class="h-4 w-4" />
					{:else}
						<IconBranch class="h-4 w-4" />
					{/if}
				</div>
			</button>
		</Tooltip>
		{#if expanded}
			<div class="flex-grow font-mono font-bold">{base.branchName}</div>
		{/if}
	</div>

	<div class="relative flex flex-grow justify-center overflow-y-scroll">
		<div class="w-5" />

		<div
			class="
			dark:form-dark-600
			w-px bg-gradient-to-b from-transparent via-light-600 dark:from-dark-400 dark:via-dark-400"
		/>
		<div class="flex-grow" />

		{#if expanded}
			<div class="w-60 py-4 pr-4">
				<div class="ml-4">
					<Tooltip
						label={'Merges the commits from ' +
							base.branchName +
							' into the base of all applied virtual branches'}
					>
						<Button
							width="full-width"
							height="small"
							color="purple"
							on:click={updateTargetModal.show}
						>
							Merge into common base
						</Button>
					</Tooltip>
				</div>
				<div class="-ml-[18px] flex h-full">
					<div class="z-40 mt-4 flex w-full flex-col gap-2">
						{#each base.upstreamCommits as commit}
							<div class="flex w-full items-center pb-2">
								<div class="ml-3 mr-2">
									<div
										class="h-3 w-3 rounded-full border-2 border-light-600 bg-light-600 dark:border-dark-400 dark:bg-dark-400"
										class:bg-light-500={commit.isRemote}
										class:dark:bg-dark-500={commit.isRemote}
									/>
								</div>
								<CommitCard {commit} {base} />
							</div>
						{/each}
					</div>
				</div>
			</div>
		{:else}
			<div class="w-5" />
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
