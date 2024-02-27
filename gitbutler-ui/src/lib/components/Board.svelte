<script lang="ts" async="true">
	import FullscreenLoading from './FullscreenLoading.svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import BranchLane from '$lib/components/BranchLane.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import ImgThemed from '$lib/components/ImgThemed.svelte';
	import { cloneWithRotation } from '$lib/dragging/draggable';
	import { open } from '@tauri-apps/api/shell';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import type { Project } from '$lib/backend/projects';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';

	export let project: Project;
	export let projectPath: string;

	export let branches: Branch[] | undefined;
	export let branchesError: any;

	export let base: BaseBranch | undefined | null;

	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let branchController: BranchController;
	export let branchService: BranchService;
	export let githubService: GitHubService;

	export let user: User | undefined;

	let dragged: any;
	let dropZone: HTMLDivElement;
	let priorPosition = 0;
	let dropPosition = 0;

	let dragHandle: any;
	let clone: any;
</script>

{#if branchesError}
	<div class="p-4" data-tauri-drag-region>Something went wrong...</div>
{:else if !branches}
	<FullscreenLoading />
{:else}
	<div
		class="board"
		role="group"
		data-tauri-drag-region
		bind:this={dropZone}
		on:dragover={(e) => {
			if (!dragged) return;

			e.preventDefault();
			const children = [...e.currentTarget.children];
			dropPosition = 0;
			// We account for the NewBranchDropZone by subtracting 2
			for (let i = 0; i < children.length - 2; i++) {
				const pos = children[i].getBoundingClientRect();
				if (e.clientX > pos.right + dragged.offsetWidth / 2) {
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
		{#each branches.sort((a, b) => a.order - b.order) as branch (branch.id)}
			<!-- svelte-ignore a11y-no-static-element-interactions -->
			<div
				class="h-full"
				draggable="true"
				on:mousedown={(e) => (dragHandle = e.target)}
				on:dragstart={(e) => {
					if (dragHandle.dataset.dragHandle == undefined) {
						// We rely on elements with id `drag-handle` to initiate this drag
						e.preventDefault();
						e.stopPropagation();
						return;
					}
					clone = cloneWithRotation(e.target);
					document.body.appendChild(clone);
					// Get chromium to fire dragover & drop events
					// https://stackoverflow.com/questions/6481094/html5-drag-and-drop-ondragover-not-firing-in-chrome/6483205#6483205
					e.dataTransfer?.setData('text/html', 'd'); // cannot be empty string
					e.dataTransfer?.setDragImage(clone, e.offsetX + 30, e.offsetY + 30); // Adds the padding
					dragged = e.currentTarget;
					priorPosition = Array.from(dropZone.children).indexOf(dragged);
				}}
				on:dragend={() => {
					dragged = undefined;
					clone?.remove();
				}}
			>
				<BranchLane
					{branch}
					{project}
					{base}
					{cloud}
					{branchController}
					{branchService}
					branchCount={branches.filter((c) => c.active).length}
					{projectPath}
					{user}
					{githubService}
				></BranchLane>
			</div>
		{/each}

		{#if branches.length == 0}
			<div class="empty-board">
				<div class="empty-board__image">
					<ImgThemed
						imgSet={{
							light: '/images/dzen-pc-light.webp',
							dark: '/images/dzen-pc-dark.webp'
						}}
					/>
				</div>

				<div class="empty-board__about">
					<h3 class="text-serif-40">You are up to date</h3>
					<p class="text-base-body-14">
						Your working directory matches the base branch.
						<br />
						Any edits auto-create a virtual branch for easy management.
					</p>
				</div>

				<div class="empty-board__suggestions">
					<div class="empty-board__suggestions__block">
						<h3 class="text-base-14 text-bold">Start</h3>
						<div class="empty-board__suggestions__links">
							<a
								class="empty-board__suggestions__link"
								target="_blank"
								rel="noreferrer"
								href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes"
							>
								<div class="empty-board__suggestions__link__icon">
									<Icon name="docs" />
								</div>

								<span class="text-base-12">GitButler Docs</span>
							</a>
							<div
								class="empty-board__suggestions__link"
								role="button"
								tabindex="0"
								on:keypress={() => open(`vscode://file${projectPath}/`)}
								on:click={() => open(`vscode://file${projectPath}/`)}
							>
								<div class="empty-board__suggestions__link__icon">
									<Icon name="vscode" />
								</div>
								<span class="text-base-12">Open in VSCode</span>
							</div>
						</div>
					</div>

					<div class="empty-board__suggestions__block">
						<h3 class="text-base-14 text-bold">Recent commits</h3>
						<div class="empty-board__suggestions__links">
							{#each (base?.recentCommits || []).slice(0, 4) as commit}
								<a
									class="empty-board__suggestions__link"
									href={base?.commitUrl(commit.id)}
									target="_blank"
									rel="noreferrer"
									title="Open in browser"
								>
									<div class="empty-board__suggestions__link__icon">
										<Icon name="commit" />
									</div>

									<span class="text-base-12">{commit.description}</span>
								</a>
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
	}

	/* Empty board */

	.empty-board {
		user-select: none;
		display: flex;
		flex-direction: column;
		margin: auto;
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		border-radius: var(--radius-l);
		width: 86%;
		max-width: 45rem;
		padding: var(--space-32);
	}

	.empty-board__image {
		width: 14.125rem;
		margin-top: var(--space-10);
		margin-bottom: var(--space-20);
		transform: translateX(-3rem);
	}

	.empty-board__about {
		display: flex;
		flex-direction: column;
		margin-bottom: var(--space-32);
	}

	.empty-board__about h3 {
		color: var(--clr-theme-scale-ntrl-0);
	}

	.empty-board__about p {
		color: var(--clr-theme-scale-ntrl-40);
	}

	.empty-board__suggestions {
		display: flex;
		flex-direction: row;
		gap: var(--space-80);
	}

	.empty-board__suggestions__block {
		display: flex;
		flex-direction: column;
		gap: var(--space-16);
		min-width: 8rem;
	}

	.empty-board__suggestions__block h3 {
		color: var(--clr-theme-scale-ntrl-0);
	}

	.empty-board__suggestions__links {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
		margin-left: calc(var(--space-4) * -1);
	}

	.empty-board__suggestions__link {
		cursor: default;
		display: flex;
		width: fit-content;
		max-width: 100%;
		padding: var(--space-2) var(--space-6) var(--space-2) var(--space-4);
		border-radius: var(--radius-s);
		gap: var(--space-10);
		transition: background-color var(--transition-fast);
		overflow: hidden;

		&:hover {
			background-color: var(--clr-theme-container-pale);
		}

		& span {
			color: var(--clr-theme-scale-ntrl-40);
			margin-top: calc(var(--space-6) / 2);
			white-space: nowrap;
			text-overflow: ellipsis;
			overflow: hidden;
		}
	}

	.empty-board__suggestions__link__icon {
		color: var(--clr-theme-scale-ntrl-50);
	}
</style>
