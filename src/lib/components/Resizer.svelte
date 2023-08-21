<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let direction: 'horizontal' | 'vertical';
	export let viewport: HTMLElement;
	export let reverse = false;
	export let grow = true; // Grow beyond container on hover

	let classes = '';
	export { classes as class };
	export let width = 1;
	export let height = 1;

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
			if (!reverse) initial = e.clientX - viewport.scrollWidth;
			if (reverse) initial = window.innerWidth - e.clientX - viewport.scrollWidth;
		}
		if (direction == 'vertical') {
			if (!reverse) initial = e.clientY - viewport.scrollHeight;
			if (reverse) initial = window.innerHeight - e.clientY - viewport.scrollHeight;
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
			if (!reverse) dispatch('width', e.clientX - initial + 2); // TODO: Define `+ 2` better
			if (reverse) dispatch('width', document.body.scrollWidth - e.clientX - initial);
		}
		if (direction == 'vertical') {
			if (!reverse) dispatch('height', e.clientY - initial);
			if (reverse) dispatch('height', document.body.scrollHeight - e.clientY - initial);
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
