<script lang="ts" async="true">
	import Lane from './BranchLane.svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import { dzHighlight } from './dropZone';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { getCloudApiClient } from '$lib/api/cloud/api';
	import type { LoadState } from '@square/svelte-store';
	import { open } from '@tauri-apps/api/shell';
	import { IconFile, IconTerminal, IconExternalLink } from '$lib/icons';
	import { rectToClientRect } from 'svelte-floating-ui/core';

	export let projectId: string;
	export let projectPath: string;

	export let branches: Branch[] | undefined;
	export let branchesState: LoadState;

	export let base: BaseBranch | undefined;
	export let baseBranchState: LoadState;

	export let cloudEnabled: boolean;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let branchController: BranchController;

	let dragged: any;
	let dropZone: HTMLDivElement;
	let priorPosition = 0;
	let dropPosition = 0;

	const dzType = 'text/branch';

	function handleEmpty() {
		const emptyIndex = branches?.findIndex((item) => !item.files || item.files.length == 0);
		if (emptyIndex && emptyIndex != -1) {
			branches?.splice(emptyIndex, 1);
		}
		branches = branches;
	}

	$: activeBranches = branches?.filter((b) => b.active);
</script>

{#if branchesState.isLoading || baseBranchState.isLoading}
	<div class="p-4">Loading...</div>
{:else if branchesState.isError || baseBranchState.isError}
	<div class="p-4">Something went wrong...</div>
{:else if branches}
	<div
		bind:this={dropZone}
		id="branch-lanes"
		class="flex flex-shrink flex-grow items-start bg-light-300 dark:bg-dark-1100"
		role="group"
		use:dzHighlight={{ type: dzType, active: 'board-dz-active', hover: 'board-dz-hover' }}
		on:dragover={(e) => {
			const children = [...e.currentTarget.children];
			dropPosition = 0;
			// We account for the NewBranchDropZone by subtracting 2
			for (let i = 0; i < children.length - 2; i++) {
				const pos = children[i].getBoundingClientRect();
				if (e.clientX > pos.left + pos.width) {
					dropPosition = i + 1; // Note that this is declared in the <script>
				} else {
					break;
				}
			}
			const idx = children.indexOf(dragged);
			if (idx != dropPosition) {
				idx >= dropPosition
					? children[dropPosition].before(dragged)
					: children[dropPosition].after(dragged);
			}
		}}
		on:drop={() => {
			if (!branches) return;
			if (priorPosition != dropPosition) {
				const el = branches.splice(priorPosition, 1);
				branches.splice(dropPosition, 0, ...el);
				branches.forEach((branch, i) => {
					if (branch.order !== i) {
						branchController.updateBranchOrder(branch.id, i);
					}
				});
			}
		}}
	>
		{#each branches.filter((c) => c.active) as branch (branch.id)}
			<Lane
				on:dragstart={(e) => {
					if (!e.dataTransfer) return;
					e.dataTransfer.setData(dzType, branch.id);
					dragged = e.currentTarget;
					priorPosition = Array.from(dropZone.children).indexOf(dragged);
				}}
				on:empty={handleEmpty}
				{branch}
				{projectId}
				{projectPath}
				{base}
				{cloudEnabled}
				{cloud}
				{branchController}
			/>
		{/each}

		{#if !activeBranches || activeBranches.length == 0}
			<div
				class="m-auto mx-10 flex w-full flex-grow items-center justify-center rounded border border-light-400 bg-light-200 p-8 dark:border-dark-500 dark:bg-dark-1000"
			>
				<div class="inline-flex w-[35rem] flex-col items-center gap-y-4">
					<h3 class="text-xl font-medium">You are up to date</h3>
					<p class="text-light-700 dark:text-dark-200">
						This means that your working directory looks exactly like your base branch. There isn't
						anything locally that is not in your production code!
					</p>
					<p class="text-light-700 dark:text-dark-200">
						If you start editing files in your working directory, a new virtual branch will
						automatically be created and you can manage it here.
					</p>
					<div class="flex w-full">
						<div class="w-1/2">
							<h3 class="mb-2 text-xl font-medium">Start</h3>
							<div class="flex flex-col gap-1 text-light-700 dark:text-dark-200">
								<a
									class="inline-flex items-center gap-2 hover:text-light-800 dark:hover:text-dark-100"
									target="_blank"
									rel="noreferrer"
									href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes"
								>
									<IconFile class="h-4 w-4" />
									GitButler Docs
								</a>
								<div
									class="inline-flex items-center gap-2 hover:text-light-800 dark:hover:text-dark-100"
									role="button"
									tabindex="0"
									on:keypress={() => open(`vscode://file${projectPath}/`)}
									on:click={() => open(`vscode://file${projectPath}/`)}
								>
									<IconTerminal class="h-4 w-4" />
									Open in VSCode
								</div>
							</div>
						</div>
						<div class="w-1/2">
							<h3 class="mb-2 text-xl font-medium">Recent</h3>
							{#each (base?.recentCommits || []).slice(0, 4) as commit}
								<div class="w-full truncate">
									<a
										class="inline-flex items-center gap-2 text-light-700 hover:text-light-800 dark:text-dark-200 hover:dark:text-dark-100
										 "
										href={base?.commitUrl(commit.id)}
										target="_blank"
										rel="noreferrer"
										title="Open in browser"
									>
										<IconExternalLink class="h-4 w-4" />
										{commit.description}
									</a>
								</div>
							{/each}
						</div>
					</div>
				</div>
			</div>
		{:else}
			<NewBranchDropZone {branchController} />
		{/if}
	</div>
{/if}
