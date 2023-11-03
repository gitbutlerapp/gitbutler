<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	let classes = '';
	export { classes as class };

	// The element that is being resized
	export let viewport: HTMLElement;

	// Sets direction of resizing for viewport
	export let direction: 'horizontal' | 'vertical';

	// For resizing bottom-up or right-to-left
	export let reverse = false;

	// Grow beyond container on hover
	export let grow = true;

	// Width of resize handle when horizontal
	export let width = 1;

	// Height of resize handle when vertical
	export let height = 1;

	// Min width of viewport when horizontal
	export let minWidth = 100;

	// min height of viewport when vertical
	export let minHeight = 100;

	let dragging = false;
	let hovering = false;
	let initial = 0;

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
		dragging = true;

		if (direction == 'horizontal') {
			if (!reverse) initial = e.clientX - viewport.clientWidth;
			if (reverse) initial = window.innerWidth - e.clientX - viewport.clientWidth;
		}
		if (direction == 'vertical') {
			if (!reverse) initial = e.clientY - viewport.clientHeight;
			if (reverse) initial = window.innerHeight - e.clientY - viewport.clientHeight;
		}

		dispatch('resizing', true);
	}

	function onMouseEnter() {
		hovering = true;
	}

	function onMouseLeave() {
		if (!dragging) {
			hovering = false;
		}
	}

	function onMouseMove(e: MouseEvent) {
		if (direction == 'horizontal') {
			let width = !reverse
				? e.clientX - initial + 2
				: document.body.scrollWidth - e.clientX - initial;
			dispatch('width', minWidth ? Math.max(minWidth, width) : width);
		}
		if (direction == 'vertical') {
			let height = !reverse
				? e.clientY - initial
				: document.body.scrollHeight - e.clientY - initial;
			dispatch('height', minHeight ? Math.max(minHeight, height) : height);
		}
	}

	function onMouseUp() {
		dragging = false;
		hovering = false;
		document.removeEventListener('mouseup', onMouseUp);
		document.removeEventListener('mousemove', onMouseMove);
		dispatch('resizing', false);
	}
</script>

<div
	on:mousedown={onMouseDown}
	on:mouseenter={onMouseEnter}
	on:mouseleave={onMouseLeave}
	tabindex="0"
	role="slider"
	aria-valuenow={viewport?.clientHeight}
	class:bg-orange-300={hovering}
	class:dark:bg-orange-700={hovering}
	class:cursor-ew-resize={hovering && direction == 'horizontal'}
	class:cursor-ns-resize={hovering && direction == 'vertical'}
	class:-mt-[2px]={hovering && grow && direction == 'vertical'}
	class:-mb-[2px]={hovering && grow && direction == 'vertical'}
	class:-mr-[2px]={hovering && grow && direction == 'horizontal'}
	class:-ml-[2px]={hovering && grow && direction == 'horizontal'}
	class:h-full={direction == 'vertical'}
	style:height={direction == 'vertical'
		? hovering
			? grow
				? `${height + 4}px`
				: `${height}px`
			: `${height}px`
		: undefined}
	style:width={direction == 'horizontal'
		? hovering
			? grow
				? `${width + 4}px`
				: `${width}px`
			: `${width}px`
		: undefined}
	class="shrink-0 {classes ? ` ${classes}` : ''}"
/>
