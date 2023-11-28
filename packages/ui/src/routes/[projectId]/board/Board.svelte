<script lang="ts" async="true">
	import BranchLane from '../components/BranchLane.svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import { open } from '@tauri-apps/api/shell';
	import { IconFile, IconTerminal, IconExternalLink } from '$lib/icons';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import type { PrService } from '$lib/github/pullrequest';

	export let projectId: string;
	export let projectPath: string;

	export let branches: Branch[] | undefined;
	export let branchesError: any;

	export let base: BaseBranch | undefined | null;

	export let cloudEnabled: boolean;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let branchController: BranchController;
	export let prService: PrService;

	export let githubContext: GitHubIntegrationContext | undefined;
	export let user: User | undefined;

	let dragged: any;
	let dropZone: HTMLDivElement;
	let priorPosition = 0;
	let dropPosition = 0;

	$: activeBranches = branches?.filter((b) => b.active) || [];
</script>

{#if branchesError}
	<div class="p-4">Something went wrong...</div>
{:else if !branches}
	<div class="p-4">Loading...</div>
{:else}
	<div
		class="board"
		role="group"
		bind:this={dropZone}
		on:dragover={(e) => {
			if (!dragged) return;

			e.preventDefault();
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
		on:drop={(e) => {
			if (!dragged) return;
			if (!branches) return;
			e.preventDefault();
			if (priorPosition != dropPosition) {
				const el = activeBranches.splice(priorPosition, 1);
				activeBranches.splice(dropPosition, 0, ...el);
				activeBranches.forEach((branch, i) => {
					if (branch.order !== i) {
						branchController.updateBranchOrder(branch.id, i);
					}
				});
			}
		}}
	>
		{#each activeBranches.sort((a, b) => a.order - b.order) as branch (branch.id)}
			<div
				class="h-full"
				role="group"
				draggable="true"
				on:dragstart={(e) => {
					dragged = e.currentTarget;
					priorPosition = Array.from(dropZone.children).indexOf(dragged);
				}}
				on:dragend={() => {
					dragged = undefined;
				}}
			>
				<BranchLane
					{branch}
					{projectId}
					{base}
					{cloudEnabled}
					{cloud}
					{branchController}
					branchCount={branches.filter((c) => c.active).length}
					{githubContext}
					{projectPath}
					{user}
					{prService}
				/>
			</div>
		{/each}

		{#if !activeBranches || activeBranches.length == 0}
			<div
				class="text-color-2 m-auto mx-10 flex w-full flex-grow items-center justify-center rounded border p-8"
				style:background-color="var(--bg-surface-highlight)"
				style:border-color="var(--border-surface)"
			>
				<div class="inline-flex w-[35rem] flex-col items-center gap-y-4">
					<h3 class="text-xl font-medium">You are up to date</h3>
					<p class="text-color-3">
						This means that your working directory looks exactly like your base branch. There isn't
						anything locally that is not in your production code!
					</p>
					<p class="text-color-3">
						If you start editing files in your working directory, a new virtual branch will
						automatically be created and you can manage it here.
					</p>
					<div class="flex w-full">
						<div class="w-1/2">
							<h3 class="mb-2 text-xl font-medium">Start</h3>
							<div class="text-color-3 flex flex-col gap-1">
								<a
									class="hover:text-color-1 inline-flex items-center gap-2"
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
								<div class="text-color-3 w-full truncate">
									<a
										class="hover:text-color-2 inline-flex items-center gap-2"
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

<style lang="postcss">
	.board {
		display: flex;
		flex-grow: 1;
		flex-shrink: 1;
		align-items: flex-start;
		height: 100%;
		padding: var(--space-16);
		gap: var(--space-16);
	}
</style>
