<script lang="ts" async="true">
	import FullviewLoading from './FullviewLoading.svelte';
	import dzenSvg from '$lib/assets/dzen-pc.svg?raw';
	import { Project } from '$lib/backend/projects';
	import BranchLane from '$lib/branch/BranchLane.svelte';
	import BranchDropzone from '$lib/components/BranchDropzone.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { cloneWithRotation } from '$lib/dragging/draggable';
	import { persisted } from '$lib/persisted/persisted';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { editor } from '$lib/utils/systemEditor';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BaseBranch } from '$lib/vbranches/types';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { open } from '@tauri-apps/api/shell';

	const vbranchService = getContext(VirtualBranchService);
	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const project = getContext(Project);
	const activeBranchesError = vbranchService.activeBranchesError;
	const activeBranches = vbranchService.activeBranches;
	const showHistoryView = persisted(false, 'showHistoryView');

	let dragged: any;
	let dropZone: HTMLDivElement;
	let priorPosition = 0;
	let dropPosition = 0;

	let dragHandle: any;
	let clone: any;
	$: if ($activeBranchesError) {
		$showHistoryView = true;
	}
</script>

{#if $activeBranchesError}
	<div class="p-4" data-tauri-drag-region>Something went wrong...</div>
{:else if !$activeBranches}
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
			if (idx !== dropPosition) {
				idx >= dropPosition
					? children[dropPosition].before(dragged)
					: children[dropPosition].after(dragged);
			}
		}}
		on:drop={(e) => {
			if (!dragged) return;
			if (!$activeBranches) return;
			e.preventDefault();
			if (priorPosition !== dropPosition) {
				const el = $activeBranches.splice(priorPosition, 1);
				$activeBranches.splice(dropPosition, 0, ...el);
				$activeBranches.forEach((branch, i) => {
					if (branch.order !== i) {
						branchController.updateBranchOrder(branch.id, i);
					}
				});
			}
		}}
	>
		{#each $activeBranches.sort((a, b) => a.order - b.order) as branch (branch.id)}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="branch draggable-branch"
				draggable="true"
				on:mousedown={(e) => (dragHandle = e.target)}
				on:dragstart={(e) => {
					if (dragHandle.dataset.dragHandle === undefined) {
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
				<BranchLane {branch} />
			</div>
		{/each}

		{#if $activeBranches.length === 0}
			<div
				data-tauri-drag-region
				class="empty-board__wrapper"
				class:transition-fly={$activeBranches.length === 0}
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

										<span class="text-base-12">GitButler Docs</span>
									</a>
									<div
										class="empty-board__suggestions__link"
										role="button"
										tabindex="0"
										on:keypress={async () =>
											await open(`${editor.get()}://file${project.vscodePath}/?windowId=_blank`)}
										on:click={async () =>
											await open(`${editor.get()}://file${project.vscodePath}/?windowId=_blank`)}
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
			<BranchDropzone />
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

	.branch {
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
		padding: 0 40px;
	}

	.empty-board {
		display: flex;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		width: 100%;
		gap: 48px;
		max-width: 736px;
		min-height: 320px;
		padding: 32px;
	}

	.empty-board__content {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		padding-left: 4px;
	}

	.empty-board__image-frame {
		flex-shrink: 0;
		position: relative;
		width: 180px;
		height: auto;
		border-radius: var(--radius-l);
		background-color: var(--clr-illustration-bg);

		&::before {
			content: '';
			display: block;
			position: absolute;
			bottom: 12%;
			left: 50%;
			width: 104px;
			height: 24px;
			transform: translateX(-50%) scale(1.15);
			border-radius: 100%;
			background-color: var(--clr-illustration-outline);
			opacity: 0.1;
			animation-delay: 3s;
		}
	}

	.empty-board__image {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -66%) translateZ(0);
		width: 212px;
	}

	.empty-board__about {
		display: flex;
		flex-direction: column;
		margin-bottom: 32px;
	}

	.empty-board__about h3 {
		color: var(--clr-scale-ntrl-0);
	}

	.empty-board__about p {
		color: var(--clr-scale-ntrl-40);
	}

	.empty-board__suggestions {
		display: flex;
		flex-direction: row;
		gap: 40px;
	}

	.empty-board__suggestions__block {
		display: flex;
		flex-direction: column;
		gap: 16px;
		min-width: 128px;
	}

	.empty-board__suggestions__block h3 {
		color: var(--clr-scale-ntrl-0);
	}

	.empty-board__suggestions__links {
		display: flex;
		flex-direction: column;
		gap: 6px;
		margin-left: -4px;
	}

	.empty-board__suggestions__link {
		cursor: default;
		display: flex;
		width: fit-content;
		max-width: 100%;
		padding: 2px 6px 2px 4px;
		border-radius: var(--radius-s);
		gap: 10px;
		transition: background-color var(--transition-fast);
		overflow: hidden;

		&:hover {
			background-color: var(--clr-bg-2);
		}

		& span {
			color: var(--clr-scale-ntrl-40);
			margin-top: 3px;
			white-space: nowrap;
			text-overflow: ellipsis;
			overflow: hidden;
		}
	}

	.empty-board__suggestions__link__icon {
		color: var(--clr-scale-ntrl-50);
	}
</style>
