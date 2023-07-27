<script lang="ts">
	import { IconBranch, IconRefresh, IconGithub } from '$lib/icons';
	import { Button, Modal, Tooltip } from '$lib/components';
	import type { BaseBranch } from '$lib/vbranches';
	import CommitCard from './CommitCard.svelte';
	import type { BranchController } from '$lib/vbranches';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';
	import { getContext } from 'svelte';
	import Scrollbar from '$lib/components/Scrollbar.svelte';

	export let base: BaseBranch;

	let updateTargetModal: Modal;
	let viewport: Element;
	let contents: Element;

	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);

	// $: behind = baseBranch.behind > 0;
	$: behindMessage = base.behind > 0 ? `behind ${base.behind}` : 'up-to-date';

	let fetching = false;
	$: expanded = true;
	let buttonHovered = false;
</script>

<div
	class="flex h-full shrink-0 cursor-default snap-center flex-col overflow-y-hidden
	overscroll-y-none border-light-600
	bg-light-150
	dark:border-l
	dark:border-dark-600 dark:border-r-light-800 dark:bg-dark-700 dark:text-dark-100"
	role="group"
>
	<div class="flex w-full bg-light-200 dark:bg-dark-800">
		<div
			class="flex flex-grow items-center border-b border-light-500 pl-1 dark:border-dark-500"
			class:border-r={!expanded}
			class:pr-1={!expanded}
		>
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
				<div class="flex-grow pl-2 font-mono font-bold">
					{base.branchName}
				</div>
			{/if}
		</div>
		{#if expanded}
			<div
				class="h-full w-3 shrink-0 border-l border-r border-light-400 bg-light-300 dark:bg-dark-500 dark:border-dark-600"
			/>
		{/if}
	</div>

	<div class="relative flex flex-grow justify-center overflow-hidden">
		<div class="flex-grow" />
		{#if expanded}
			<div
				class="relative flex h-full w-60 shrink-0 flex-grow cursor-default snap-center flex-col overflow-y-hidden overscroll-y-none border-light-300 bg-light-200 pr-2 dark:border-l dark:border-dark-600 dark:border-r-light-800 dark:bg-dark-700 dark:text-dark-100"
			>
				<div
					bind:this={viewport}
					class="hide-native-scrollbar flex max-h-full flex-grow flex-col overflow-y-scroll pb-8 pt-2"
				>
					<div bind:this={contents}>
						<div class="ml-8">
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
						<div class="flex h-full">
							<div class="z-40 mt-2 flex w-full flex-col gap-2">
								{#each base.upstreamCommits as commit}
									<div class="flex w-full items-center">
										<div class="ml-3 mr-3">
											<div
												class="h-2 w-2 rounded-full border-2 border-light-500 bg-light-500 dark:border-dark-400 dark:bg-dark-400"
												class:bg-light-500={commit.isRemote}
												class:dark:bg-dark-500={commit.isRemote}
											/>
										</div>
										<CommitCard {commit} url={base.commitUrl(commit.id)} />
									</div>
								{/each}
							</div>
						</div>
					</div>
				</div>
			</div>
			<div
				class="w-3 shrink-0 border-l border-r border-light-400 bg-light-300 dark:border-dark-500 dark:bg-dark-500"
			>
				<Scrollbar {viewport} {contents} width="0.75rem" opacity="0.1" />
			</div>
		{:else}
			<div class="w-5" />
		{/if}
		<div
			class="absolute left-[1rem] h-full w-px bg-gradient-to-b from-transparent via-light-600 dark:from-dark-400 dark:via-dark-400"
		/>
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
