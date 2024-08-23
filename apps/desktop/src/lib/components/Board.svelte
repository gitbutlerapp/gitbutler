<script lang="ts" async="true">
	import FullviewLoading from './FullviewLoading.svelte';
	import dzenSvg from '$lib/assets/dzen-pc.svg?raw';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import BranchDropzone from '$lib/branch/BranchDropzone.svelte';
	import BranchLane from '$lib/branch/BranchLane.svelte';
	import { cloneElement } from '$lib/dragging/draggable';
	import { editor } from '$lib/editorLink/editorLink';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { persisted } from '$lib/persisted/persisted';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { throttle } from '$lib/utils/misc';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { open } from '@tauri-apps/api/shell';
	import { flip } from 'svelte/animate';

	const vbranchService = getContext(VirtualBranchService);
	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const project = getContext(Project);
	const error = vbranchService.error;
	const branches = vbranchService.branches;
	const showHistoryView = persisted(false, 'showHistoryView');
	const gitHost = getGitHost();

	let dragged: HTMLDivElement | undefined;
	let dropZone: HTMLDivElement;

	let dragHandle: any;
	let clone: any;
	$: if ($error) {
		$showHistoryView = true;
	}
	$: sortedBranches = $branches?.sort((a, b) => a.order - b.order) || [];

	async function openInVSCode() {
		open(`${$editor}://file${project.vscodePath}/?windowId=_blank`);
	}

	const handleDragOver = throttle((e: MouseEvent & { currentTarget: HTMLDivElement }) => {
		e.preventDefault();
		if (!dragged) {
			return; // Something other than a lane is being dragged.
		}

		const children = Array.from(e.currentTarget.children);
		const currentPosition = children.indexOf(dragged);

		let dropPosition = 0;
		let mouseLeft = e.clientX - dropZone.getBoundingClientRect().left;
		let cumulativeWidth = dropZone.offsetLeft;

		for (let i = 0; i < children.length; i++) {
			if (i === currentPosition) {
				continue;
			}

			const childWidth = (children[i] as HTMLElement).offsetWidth;
			if (mouseLeft > cumulativeWidth + childWidth / 2) {
				// New position depends on drag direction.
				dropPosition = i < currentPosition ? i + 1 : i;
				cumulativeWidth += childWidth;
			} else {
				break;
			}
		}

		// Update sorted branch array manually.
		if (currentPosition !== dropPosition) {
			const el = sortedBranches.splice(currentPosition, 1);
			sortedBranches.splice(dropPosition, 0, ...el);
			sortedBranches = sortedBranches; // Redraws #each loop.
		}
	}, 200);
</script>

{#if $error}
	<div class="p-4" data-tauri-drag-region>Something went wrong...</div>
{:else if !$branches}
	<FullviewLoading />
{:else}
	<div
		class="board"
		role="group"
		data-tauri-drag-region
		on:drop={(e) => {
			e.preventDefault();
			if (!dragged) {
				return; // Something other than a lane was dropped.
			}
			branchController.updateBranchOrder(sortedBranches.map((b, i) => ({ id: b.id, order: i })));
		}}
	>
		<div
			role="group"
			class="branches"
			data-tauri-drag-region
			bind:this={dropZone}
			on:dragover={(e) => handleDragOver(e)}
		>
			{#each sortedBranches as branch (branch.id)}
				<div
					role="presentation"
					aria-label="Branch"
					tabindex="-1"
					class="branch draggable-branch"
					draggable="true"
					animate:flip={{ duration: 150 }}
					on:mousedown={(e) => (dragHandle = e.target)}
					on:dragstart={(e) => {
						if (dragHandle.dataset.dragHandle === undefined) {
							// We rely on elements with id `drag-handle` to initiate this drag
							e.preventDefault();
							e.stopPropagation();
							return;
						}
						clone = cloneElement(e.currentTarget);
						document.body.appendChild(clone);
						// Get chromium to fire dragover & drop events
						// https://stackoverflow.com/questions/6481094/html5-drag-and-drop-ondragover-not-firing-in-chrome/6483205#6483205
						e.dataTransfer?.setData('text/html', 'd'); // cannot be empty string
						e.dataTransfer?.setDragImage(clone, e.offsetX, e.offsetY); // Adds the padding
						dragged = e.currentTarget;
						dragged.style.opacity = '0.6';
					}}
					on:dragend={() => {
						if (dragged) {
							dragged.style.opacity = '1';
							dragged = undefined;
						}
						clone?.remove();
					}}
				>
					<BranchLane {branch} />
				</div>
			{/each}
		</div>

		{#if $branches.length === 0}
			<div
				data-tauri-drag-region
				class="empty-board__wrapper"
				class:transition-fly={$branches.length === 0}
			>
				<div class="empty-board">
					<div class="empty-board__content">
						<div class="empty-board__about">
							<h3 class="text-serif-40 text-body">You're up to date</h3>
							<p class="text-14 text-body">
								Your working directory matches the base branch.
								<br />
								Any edits auto-create a virtual branch for easy management.
							</p>
						</div>

						<div class="empty-board__suggestions">
							<div class="empty-board__suggestions__block">
								<h3 class="text-14 text-bold">Start</h3>
								<div class="empty-board__suggestions__links">
									<div
										class="empty-board__suggestions__link"
										role="button"
										tabindex="0"
										on:keypress={async () => await branchController.createBranch({})}
										on:click={async () => await branchController.createBranch({})}
									>
										<div class="empty-board__suggestions__link__icon">
											<Icon name="new-branch" />
										</div>
										<span class="text-12">Create a new branch</span>
									</div>
									<a
										class="empty-board__suggestions__link"
										target="_blank"
										rel="noreferrer"
										href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes"
									>
										<div class="empty-board__suggestions__link__icon">
											<Icon name="docs" />
										</div>

										<span class="text-12">GitButler Docs</span>
									</a>
									<div
										class="empty-board__suggestions__link"
										role="button"
										tabindex="0"
										on:keypress={async () => await openInVSCode()}
										on:click={async () => await openInVSCode()}
									>
										<div class="empty-board__suggestions__link__icon">
											<Icon name="vscode" />
										</div>
										<span class="text-12">Open in VSCode</span>
									</div>
								</div>
							</div>

							<div class="empty-board__suggestions__block">
								<h3 class="text-14 text-bold">Recent commits</h3>
								<div class="empty-board__suggestions__links">
									{#each ($baseBranch?.recentCommits || []).slice(0, 4) as commit}
										<a
											class="empty-board__suggestions__link"
											href={$gitHost?.commitUrl(commit.id)}
											target="_blank"
											rel="noreferrer"
											title="Open in browser"
										>
											<div class="empty-board__suggestions__link__icon">
												<Icon name="commit" />
											</div>

											<span class="text-12">{commit.description}</span>
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

	.branches {
		display: flex;
		flex-shrink: 0;
		align-items: flex-start;
		height: 100%;
	}

	.branch {
		height: 100%;
		width: fit-content;
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
		line-height: 160%;
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
		min-width: 160px;
	}

	.empty-board__suggestions__block h3 {
		color: var(--clr-scale-ntrl-0);
	}

	.empty-board__suggestions__links {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin-left: -4px;
	}

	.empty-board__suggestions__link {
		cursor: pointer;
		display: flex;
		align-items: center;
		width: fit-content;
		max-width: 100%;
		padding: 6px;
		border-radius: var(--radius-s);
		gap: 10px;
		transition: background-color var(--transition-fast);
		overflow: hidden;

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		& span {
			color: var(--clr-scale-ntrl-40);
			white-space: nowrap;
			text-overflow: ellipsis;
			overflow: hidden;
		}
	}

	.empty-board__suggestions__link__icon {
		display: flex;
		color: var(--clr-scale-ntrl-50);
	}
</style>
