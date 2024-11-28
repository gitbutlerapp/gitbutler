<script lang="ts">
	import BoardEmptyState from './BoardEmptyState.svelte';
	import FullviewLoading from './FullviewLoading.svelte';
	import BranchDropzone from '$lib/branch/BranchDropzone.svelte';
	import BranchLane from '$lib/branch/BranchLane.svelte';
	import { showHistoryView } from '$lib/config/config';
	import { cloneElement } from '$lib/dragging/draggable';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { throttle } from '$lib/utils/misc';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { getContext } from '@gitbutler/shared/context';
	import posthog from 'posthog-js';
	import { onMount } from 'svelte';
	import { flip } from 'svelte/animate';
	import { run } from 'svelte/legacy';
	import type { VirtualBranch } from '$lib/vbranches/types';

	const vbranchService = getContext(VirtualBranchService);
	const branchController = getContext(BranchController);
	const error = vbranchService.error;
	const branches = vbranchService.branches;

	let dragged = $state<HTMLDivElement>();
	let dropZone = $state<HTMLDivElement>();

	let dragHandle: any = $state();
	let clone: any = $state();
	run(() => {
		if ($error) {
			$showHistoryView = true;
		}
	});
	let sortedBranches = $state<VirtualBranch[]>([]);
	run(() => {
		sortedBranches = $branches?.sort((a, b) => a.order - b.order) || [];
	});

	const handleDragOver = throttle((e: MouseEvent & { currentTarget: HTMLDivElement }) => {
		e.preventDefault();
		if (!dragged) {
			return; // Something other than a lane is being dragged.
		}

		const children = Array.from(e.currentTarget.children);
		const currentPosition = children.indexOf(dragged);

		let dropPosition = 0;
		let mouseLeft = e.clientX - (dropZone?.getBoundingClientRect().left ?? 0);
		let cumulativeWidth = dropZone?.offsetLeft ?? 0;

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

	const handleKeyDown = createKeybind({
		'$mod+Shift+H': () => {
			$showHistoryView = !$showHistoryView;
		},
		'$mod+z': () => {
			$showHistoryView = !$showHistoryView;
		}
	});

	onMount(() => {
		posthog.capture('Workspace Open');
	});
</script>

<svelte:window onkeydown={handleKeyDown} />
{#if $error}
	<div>Something went wrong...</div>
{:else if !$branches}
	<FullviewLoading />
{:else}
	<div
		class="board"
		role="group"
		ondrop={(e) => {
			e.preventDefault();
			if (!dragged) {
				return; // Something other than a lane was dropped.
			}
			branchController.updateBranchOrder(sortedBranches.map((b, i) => ({ id: b.id, order: i })));
		}}
	>
		<div role="group" class="branches" bind:this={dropZone} ondragover={(e) => handleDragOver(e)}>
			{#each sortedBranches as branch (branch.id)}
				<div
					role="presentation"
					aria-label="Branch"
					tabindex="-1"
					class="branch draggable-branch"
					draggable="true"
					animate:flip={{ duration: 150 }}
					onmousedown={(e) => (dragHandle = e.target)}
					ondragstart={(e) => {
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
					ondragend={() => {
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
			<BoardEmptyState />
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
		/* disable lane outline on modal close */
		outline: none;
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
</style>
