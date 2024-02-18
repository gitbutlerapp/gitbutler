<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	// The element that is being resized
	export let viewport: HTMLElement;

	// Sets direction of resizing for viewport
	export let direction: 'left' | 'right' | 'up' | 'down';

	// Needed when overflow is hidden
	export let inside = false;

	export let sticky = false;

	//
	export let minWidth = 0;
	export let minHeight = 0;

	$: orientation = ['left', 'right'].includes(direction) ? 'horizontal' : 'vertical';

	let initial = 0;
	let dragging = false;

	const dispatch = createEventDispatcher<{
		height: number;
		width: number;
		resizing: boolean;
	}>();

	function onMouseDown(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		document.addEventListener('mouseup', onMouseUp);
		document.addEventListener('mousemove', onMouseMove);

		if (direction == 'right') initial = e.clientX - viewport.clientWidth;
		if (direction == 'left') initial = window.innerWidth - e.clientX - viewport.clientWidth;
		if (direction == 'down') initial = e.clientY - viewport.clientHeight;
		if (direction == 'up') initial = window.innerHeight - e.clientY - viewport.clientHeight;

		dispatch('resizing', true);
	}

	function onMouseMove(e: MouseEvent) {
		dragging = true;
		if (direction == 'down') {
			let height = e.clientY - initial;
			dispatch('height', Math.max(height, minHeight));
		}
		if (direction == 'up') {
			let height = document.body.scrollHeight - e.clientY - initial;
			dispatch('height', Math.max(height, minHeight));
		}
		if (direction == 'right') {
			let width = e.clientX - initial + 2;
			dispatch('width', Math.max(width, minWidth));
		}
		if (direction == 'left') {
			let width = document.body.scrollWidth - e.clientX - initial;
			dispatch('width', Math.max(width, minWidth));
		}
	}

	function onMouseUp() {
		dragging = false;
		document.removeEventListener('mouseup', onMouseUp);
		document.removeEventListener('mousemove', onMouseMove);
		dispatch('resizing', false);
	}
</script>

<div
	on:mousedown={onMouseDown}
	class="resizer"
	tabindex="0"
	role="slider"
	aria-valuenow={viewport?.clientHeight}
	class:inside
	class:dragging
	class:vertical={orientation == 'vertical'}
	class:horizontal={orientation == 'horizontal'}
	class:up={direction == 'up'}
	class:down={direction == 'down'}
	class:left={direction == 'left'}
	class:right={direction == 'right'}
	class:sticky
/>

<style lang="postcss">
	.resizer {
		position: absolute;
		transition: background-color 0.1s ease-out;
		/* background-color: var(--clr-theme-container-outline-light); */
		&:hover {
			transition-delay: 0.1s;
		}
		z-index: 40;
		&:hover,
		&:focus,
		&.dragging {
			background-color: var(--clr-theme-container-outline-light);
			outline: none;
		}
	}
	.horizontal {
		width: var(--space-4);
		height: 100%;
		cursor: col-resize;
		top: 0;
		&:hover {
			width: var(--space-4);
		}
	}
	.vertical {
		height: var(--space-4);
		width: 100%;
		cursor: row-resize;
		&:hover {
			height: var(--space-4);
		}
	}
	.right {
		right: calc(-1 * var(--space-2));
		&.inside {
			right: 0;
		}
	}
	.left {
		left: 0;
		&:hover {
			width: var(--space-4);
		}
	}
	.up {
		top: 0;
		&:hover {
			height: var(--space-4);
		}
	}
	.down {
		bottom: calc(-1 * var(--space-2));
		&:hover {
			height: var(--space-4);
		}
		&.inside {
			bottom: 0;
		}
	}
	.sticky {
		position: sticky;
		top: 0;
	}
</style>
