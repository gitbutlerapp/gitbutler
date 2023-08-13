<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let direction: 'horizontal' | 'vertical';
	export let viewport: HTMLElement;

	let dragging = false;
	let hovering = false;
	let initialOffset = 0;

	const dispatch = createEventDispatcher<{
		height: number;
		width: number;
		resizing: boolean;
	}>();

	function onMouseDown(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		dragging = true;
		if (direction == 'horizontal') initialOffset = e.clientX - viewport.clientWidth;
		if (direction == 'vertical') initialOffset = e.clientY - viewport.clientHeight;
		document.addEventListener('mouseup', onMouseUp);
		document.addEventListener('mousemove', onMouseMove);
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
		if (direction == 'horizontal') dispatch('width', e.clientX - initialOffset);
		if (direction == 'vertical') dispatch('height', e.clientY - initialOffset);
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
	class:-mt-[2px]={hovering && direction == 'vertical'}
	class:-mb-[2px]={hovering && direction == 'vertical'}
	class:-mr-[2px]={hovering && direction == 'horizontal'}
	class:-ml-[2px]={hovering && direction == 'horizontal'}
	class:h-full={direction == 'vertical'}
	style:height={direction == 'vertical' ? (hovering ? '6px' : '2px') : undefined}
	style:width={direction == 'horizontal' ? (hovering ? '6px' : '2px') : undefined}
	class="z-40 shrink-0 overflow-visible bg-light-50 text-light-600 dark:bg-dark-700"
/>
