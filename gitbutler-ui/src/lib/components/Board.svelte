<script lang="ts" async="true">
	import FullviewLoading from './FullviewLoading.svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import dzenSvg from '$lib/assets/dzen-pc.svg?raw';
	import BranchLane from '$lib/components/BranchLane.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { cloneWithRotation } from '$lib/dragging/draggable';
	import { getContextByClass, getContextStoreByClass } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BaseBranch, type Branch } from '$lib/vbranches/types';
	import { open } from '@tauri-apps/api/shell';
	import type { User } from '$lib/backend/cloud';
	import type { Project } from '$lib/backend/projects';

	export let project: Project;
	export let projectPath: string;
	export let branches: Branch[] | undefined;
	export let branchesError: any;
	export let user: User | undefined;

	const branchController = getContextByClass(BranchController);
	const baseBranch = getContextStoreByClass(BaseBranch);

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
	<FullviewLoading />
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
				class="draggable-branch h-full"
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
					branchCount={branches.filter((c) => c.active).length}
					{projectPath}
					{user}
				/>
			</div>
		{/each}

		{#if branches.length == 0}
			<div
				data-tauri-drag-region
				class="empty-board__wrapper"
				class:transition-fly={branches.length == 0}
			>
				<div class="empty-board">
					<div class="empty-board__content">
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

										<span class="text-base-12">GitButler docs</span>
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
									{#each ($baseBranch?.recentCommits || []).slice(0, 4) as commit}
										<a
											class="empty-board__suggestions__link"
											href={$baseBranch?.commitUrl(commit.id)}
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

					<div data-tauri-drag-region class="empty-board__image-frame">
						<div class="empty-board__image">
							{@html dzenSvg}
						</div>
					</div>
				</div>
			</div>
		{:else}
			<NewBranchDropZone />
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

	.draggable-branch {
		/* When draggable="true" we need this to not break user-select: text in descendants.

        It has been confirmed this bug is webkit only, so for GitButler this means macos and
        most linux distributions. Why it happens we don't know, and it's somewhat unclear
        why the other draggable items don't seem suffer similar breakage.

        The problem is reproducable with the following html:
        ```
        <body style="-webkit-user-select: none; user-select: none">
            <div draggable="true">
                <p style="-webkit-user-select: text; user-select: text; cursor: text">Hello World</p>
            </div>
        </body>
        ``` */
		user-select: auto;
	}

	/* EMPTY BOARD */

	.empty-board__wrapper {
		display: flex;
		justify-content: center;
		align-items: center;
		height: 100%;
		width: 100%;
		padding: 0 var(--size-40);
	}

	.empty-board {
		display: flex;
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		border-radius: var(--radius-l);
		width: 100%;
		gap: var(--size-48);
		max-width: 46rem;
		min-height: 20rem;
		padding: var(--size-32);
	}

	.empty-board__content {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		padding-left: var(--size-4);
	}

	.empty-board__image-frame {
		flex-shrink: 0;
		position: relative;
		width: 11.2rem;
		height: auto;
		border-radius: var(--radius-l);
		background-color: var(--clr-theme-component-illustration-bg);

		&::before {
			content: '';
			display: block;
			position: absolute;
			bottom: 12%;
			left: 50%;
			width: 6.5rem;
			height: 1.5rem;
			transform: translateX(-50%) scale(1.15);
			border-radius: 100%;
			background-color: var(--clr-theme-component-illustration-outline);
			opacity: 0.08;
			animation: shadow-scale 5.5s infinite ease-in-out;
			animation-delay: 3s;
		}
	}

	.empty-board__image {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -70%) translateZ(0);
		width: 13.3rem;
		animation: hovering 5.5s infinite ease-in-out;
		animation-delay: 3s;
	}

	@keyframes hovering {
		0% {
			transform: translate(-50%, -70%) translateZ(0);
		}
		50% {
			transform: translate(-50%, -65%) translateZ(0);
		}
		100% {
			transform: translate(-50%, -70%) translateZ(0);
		}
	}

	@keyframes shadow-scale {
		0% {
			opacity: 0.08;
			transform: translateX(-50%) scale(1.15);
		}
		50% {
			opacity: 0.12;
			transform: translateX(-50%) scale(1);
		}
		100% {
			opacity: 0.08;
			transform: translateX(-50%) scale(1.15);
		}
	}

	.empty-board__about {
		display: flex;
		flex-direction: column;
		margin-bottom: var(--size-32);
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
		gap: var(--size-40);
	}

	.empty-board__suggestions__block {
		display: flex;
		flex-direction: column;
		gap: var(--size-16);
		min-width: 8rem;
	}

	.empty-board__suggestions__block h3 {
		color: var(--clr-theme-scale-ntrl-0);
	}

	.empty-board__suggestions__links {
		display: flex;
		flex-direction: column;
		gap: var(--size-6);
		margin-left: calc(var(--size-4) * -1);
	}

	.empty-board__suggestions__link {
		cursor: default;
		display: flex;
		width: fit-content;
		max-width: 100%;
		padding: var(--size-2) var(--size-6) var(--size-2) var(--size-4);
		border-radius: var(--radius-s);
		gap: var(--size-10);
		transition: background-color var(--transition-fast);
		overflow: hidden;

		&:hover {
			background-color: var(--clr-theme-container-pale);
		}

		& span {
			color: var(--clr-theme-scale-ntrl-40);
			margin-top: calc(var(--size-6) / 2);
			white-space: nowrap;
			text-overflow: ellipsis;
			overflow: hidden;
		}
	}

	.empty-board__suggestions__link__icon {
		color: var(--clr-theme-scale-ntrl-50);
	}
</style>
