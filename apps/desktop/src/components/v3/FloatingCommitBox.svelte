<script lang="ts">
	import { UiState, type FloatingCommitPosition } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { portal } from '@gitbutler/ui/utils/portal';
	import { onMount, type Snippet } from 'svelte';

	interface Props {
		branchName?: string;
		children: Snippet;
	}

	const { branchName = 'Unknown branch', children }: Props = $props();

	const uiState = getContext(UiState);

	/** Current modal position (top‑left corner, in px) */
	let x = $state(0);
	let y = $state(0);

	/** Modal dimensions */
	let width = $state(uiState.global.floatingCommitWidth.current);
	let height = $state(uiState.global.floatingCommitHeight.current);
	let floatingPosition = $state(uiState.global.floatingCommitPosition);

	/** Current snap position - tracks which snap point we're aligned to */
	let currentSnapPoint: { x: number; y: number; name: string } | null = $state(null);

	/** Internal drag state */
	let dragging = $state(false);
	let resizing = $state(false);
	let snapping = $state(false);
	let startX: number = $state(0);
	let startY: number = $state(0);
	let baseX: number = $state(0);
	let baseY: number = $state(0);
	let baseWidth: number = $state(0);
	let baseHeight: number = $state(0);

	/** The modal element – used to centre it on first render */
	let modalEl: HTMLDivElement;

	/** Snap points – populated on mount */
	let snapPoints: { x: number; y: number; name: FloatingCommitPosition }[] = [];

	/** Config */
	const MARGIN = 40; // margin from screen edge
	const MIN_WIDTH = 520;
	const MIN_HEIGHT = 330;

	function calcSnapPoints(): { x: number; y: number; name: FloatingCommitPosition }[] {
		const w = window.innerWidth;
		const h = window.innerHeight;

		return [
			// 4 Corners
			{ x: MARGIN, y: MARGIN, name: 'top-left' },
			{ x: w - MARGIN, y: MARGIN, name: 'top-right' },
			{ x: MARGIN, y: h - MARGIN, name: 'bottom-left' },
			{ x: w - MARGIN, y: h - MARGIN, name: 'bottom-right' },

			// 4 Edge Centers
			{ x: w / 2, y: MARGIN, name: 'top-center' },
			{ x: w / 2, y: h - MARGIN, name: 'bottom-center' },
			{ x: MARGIN, y: h / 2, name: 'left-center' },
			{ x: w - MARGIN, y: h / 2, name: 'right-center' },

			// Screen Center
			{ x: w / 2, y: h / 2, name: 'center' }
		];
	}

	function getAlignmentOffset(snapX: number, snapY: number) {
		const modalW = width;
		const modalH = height;

		const w = window.innerWidth;
		const h = window.innerHeight;

		let offsetX = 0;
		let offsetY = 0;

		// Determine horizontal alignment
		if (snapX <= MARGIN) {
			// Left edge
			offsetX = 0;
		} else if (snapX >= w - MARGIN) {
			// Right edge
			offsetX = -modalW;
		} else {
			// Center horizontally
			offsetX = -modalW / 2;
		}

		// Determine vertical alignment
		if (snapY <= MARGIN) {
			// Top edge
			offsetY = 0;
		} else if (snapY >= h - MARGIN) {
			// Bottom edge
			offsetY = -modalH;
		} else {
			// Center vertically
			offsetY = -modalH / 2;
		}

		return { offsetX, offsetY };
	}

	// Update your onMount function to handle window resize properly:

	onMount(() => {
		snapPoints = calcSnapPoints();

		// Find the default snap point
		const defaultSnapPoint = snapPoints.find((p) => p.name === floatingPosition.current)!;

		// Calculate position based on default snap point
		const { offsetX, offsetY } = getAlignmentOffset(defaultSnapPoint.x, defaultSnapPoint.y);
		x = defaultSnapPoint.x + offsetX;
		y = defaultSnapPoint.y + offsetY;

		// Set initial snap point
		currentSnapPoint = defaultSnapPoint;

		function handleWindowResize() {
			snapPoints = calcSnapPoints();

			if (currentSnapPoint) {
				const newSnapPoint = snapPoints.find((p) => p.name === currentSnapPoint!.name);

				if (newSnapPoint) {
					currentSnapPoint = newSnapPoint;

					const { offsetX, offsetY } = getAlignmentOffset(currentSnapPoint.x, currentSnapPoint.y);

					x = currentSnapPoint.x + offsetX;
					y = currentSnapPoint.y + offsetY;
				}
			} else {
				console.warn('No current snap point found during window resize');
			}

			const maxX = window.innerWidth - width - MARGIN;
			const maxY = window.innerHeight - height - MARGIN;

			x = Math.max(MARGIN, Math.min(x, maxX));
			y = Math.max(MARGIN, Math.min(y, maxY));
		}

		window.addEventListener('resize', handleWindowResize);

		return () => {
			window.removeEventListener('resize', handleWindowResize);
		};
	});

	// ————————————————————————— Drag helpers ——————————————————————————
	function onPointerDown(event: PointerEvent) {
		event.stopPropagation(); // Prevent triggering resize

		// Prevent if we're clicking on the resize handle
		if ((event.target as HTMLElement).closest('.resize-handle')) {
			return;
		}

		dragging = true;
		startX = event.clientX;
		startY = event.clientY;
		baseX = x;
		baseY = y;

		window.addEventListener('pointermove', onPointerMove);
		window.addEventListener('pointerup', onPointerUp, { once: true });
	}

	// In your onPointerMove function, replace the resizing logic with this:

	function onPointerMove(e: PointerEvent) {
		e.preventDefault();
		e.stopPropagation();

		if (dragging) {
			const dx = e.clientX - startX;
			const dy = e.clientY - startY;
			x = baseX + dx;
			y = baseY + dy;
		} else if (resizing && currentSnapPoint) {
			const dx = e.clientX - startX;
			const dy = e.clientY - startY;

			// For header resize handle: drag up = grow, drag down = shrink
			const newWidth = Math.max(MIN_WIDTH, baseWidth + dx);
			const newHeight = Math.max(MIN_HEIGHT, baseHeight - dy); // Inverted for header handle

			// Update dimensions

			width = newWidth;
			height = newHeight;
			uiState.global.floatingCommitWidth.current = width;
			uiState.global.floatingCommitHeight.current = height;

			// Recalculate position to maintain snap alignment
			const { offsetX, offsetY } = getAlignmentOffset(currentSnapPoint.x, currentSnapPoint.y);
			x = currentSnapPoint.x + offsetX;
			y = currentSnapPoint.y + offsetY;
		}
	}

	function onPointerUp() {
		if (dragging) {
			dragging = false;
			window.removeEventListener('pointermove', onPointerMove);

			// Calculate modal center for distance comparison
			const modalCenterX = x + width / 2;
			const modalCenterY = y + height / 2;

			// Snap to the nearest point (distance from modal center to snap point)
			const nearestSnapPoint = snapPoints.reduce((closest, p) => {
				const dist = Math.hypot(modalCenterX - p.x, modalCenterY - p.y);
				const closestDist = Math.hypot(modalCenterX - closest.x, modalCenterY - closest.y);
				return dist < closestDist ? p : closest;
			});

			const { offsetX, offsetY } = getAlignmentOffset(nearestSnapPoint.x, nearestSnapPoint.y);
			const newX = nearestSnapPoint.x + offsetX;
			const newY = nearestSnapPoint.y + offsetY;

			// Only animate if there's a significant change in position
			if (Math.abs(x - newX) > 5 || Math.abs(y - newY) > 5) {
				snapping = true;
				// Allow transition to complete before removing the snapping state
				setTimeout(() => {
					snapping = false;
				}, 300); // Match the CSS transition duration
			}

			x = newX;
			y = newY;

			// Update current snap point
			currentSnapPoint = nearestSnapPoint;

			// Update the UI state with the new position
			uiState.global.floatingCommitPosition.current = nearestSnapPoint.name;
		} else if (resizing) {
			resizing = false;
			window.removeEventListener('pointermove', onPointerMove);
		}
	}

	// ————————————————————————— Resize helpers ——————————————————————————
	function onResizePointerDown(event: PointerEvent) {
		event.stopPropagation(); // Prevent triggering drag

		resizing = true;
		startX = event.clientX;
		startY = event.clientY;
		baseWidth = width;
		baseHeight = height;

		window.addEventListener('pointermove', onPointerMove);
		window.addEventListener('pointerup', onPointerUp, { once: true });
	}

	function findBranchCardEl() {
		//  select by "data-series-name" attribute
		const branchCardEl = document.querySelector(
			`.branch-card[data-series-name="${branchName}"]`
		) as HTMLElement;

		if (branchCardEl) {
			// Scroll to the branch card element
			branchCardEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
			// add wiggle effect
			branchCardEl.classList.add('highlight-animation');
			setTimeout(() => {
				branchCardEl.classList.remove('highlight-animation');
			}, 1000); // Remove wiggle after 1 second
		} else {
			console.warn(`Branch card for "${branchName}" not found.`);
		}
	}
</script>

<div
	bind:this={modalEl}
	use:portal={'body'}
	class="modal"
	class:snapping
	class:resizing
	style="left: {x}px; top: {y}px; width: {width}px; height: {height}px;"
>
	<div class="modal-header" onpointerdown={onPointerDown}>
		<div class="drag-handle">
			<Icon name="draggable" />
		</div>
		<h4 class="text-14 text-semibold">
			Commit to <span role="presentation" class="branch-name" onclick={findBranchCardEl}
				>{branchName}</span
			>
		</h4>

		<div class="resize-handle" onpointerdown={onResizePointerDown}>
			<Icon name="resize-handle" rotate={-90} />
		</div>
	</div>
	<div class="modal-content">
		{@render children()}
	</div>
</div>

<button
	class="exit-floating-mode"
	type="button"
	onclick={() => {
		uiState.global.useFloatingCommitBox.set(false);
	}}
>
	<span class="text-12 text-semibold underline-dotted">Exit floating commit</span>
</button>

<style>
	.modal {
		display: flex;
		z-index: var(--z-floating);
		position: absolute;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-l);
		animation: slide-in 0.2s ease-out forwards;
	}

	@keyframes slide-in {
		from {
			transform: translateY(30px);
			opacity: 0;
		}
		to {
			transform: translateY(0);
			opacity: 1;
		}
	}

	.modal-header {
		display: flex;
		align-items: center;
		padding: 12px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-2);
		cursor: grab;
	}

	.modal-header h4 {
		flex: 1;
	}

	.modal.snapping {
		transition:
			left 0.3s cubic-bezier(0.4, 0, 0.2, 1),
			top 0.3s cubic-bezier(0.4, 0, 0.2, 1);
	}

	.modal.resizing {
		transition: none; /* Disable transitions while resizing */
	}

	.drag-handle,
	.resize-handle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		color: var(--clr-text-2);
	}

	.resize-handle {
		cursor: nesw-resize;
	}

	.modal-content {
		display: flex;
		flex-direction: column;
		height: 100%;
		padding: 16px;
		overflow: auto;
	}

	.branch-name {
		color: var(--clr-text-1);
		text-decoration: dotted underline;
		cursor: pointer;
	}

	.exit-floating-mode {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		padding: 12px;
		gap: 8px;
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
		cursor: pointer;
		transition: background-color 0.2s ease-in-out;

		&:hover {
			background-color: var(--clr-bg-1);
		}
	}
</style>
