<script lang="ts">
	import { FOCUS_MANAGER } from '$lib/focus/focusManager.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { on } from 'svelte/events';

	const focusManager = inject(FOCUS_MANAGER);

	$effect(() => {
		return focusManager.setCursor({ setTarget, show });
	});

	let target = $state<HTMLElement>();
	let visible = $state(false);

	function setTarget(element: HTMLElement) {
		target = element;
	}

	function show() {
		visible = true;
	}

	function isScrollable(element: HTMLElement) {
		return element.scrollHeight > element.clientHeight || element.scrollWidth > element.clientWidth;
	}

	function findNearestScrollableParent(element: HTMLElement) {
		let result = element.parentElement || document.body;

		while (true) {
			if (result === document.body) break;
			if (isScrollable(result)) break;
			result = result.parentElement || document.body;
		}

		return result;
	}

	type Rectangle = {
		left: number;
		top: number;
		width: number;
		height: number;
	};

	function overlapRectangle(a: HTMLElement, b: HTMLElement): Rectangle {
		const aRect = a.getBoundingClientRect();
		const bRect = b.getBoundingClientRect();
		const left = Math.max(aRect.left, bRect.left);
		const top = Math.max(aRect.top, bRect.top);
		const right = Math.min(aRect.right, bRect.right);
		const bottom = Math.min(aRect.bottom, bRect.bottom);

		const width = right - left;
		const height = bottom - top;

		return {
			left,
			top,
			width,
			height
		};
	}

	let left = $state(0);
	let top = $state(0);
	let width = $state(0);
	let height = $state(0);

	function setPosition(target: HTMLElement, container: HTMLElement) {
		const rect = overlapRectangle(target, container);
		left = rect.left;
		top = rect.top;
		width = rect.width;
		height = rect.height;
	}

	$inspect(target);

	$effect(() => {
		if (!target || !visible) return;
		const container = findNearestScrollableParent(target);

		setPosition(target, container);

		const unsub = on(
			container,
			'scroll',
			() => {
				console.log('hi');
				if (!target) return;
				setPosition(target, container);
			},
			{ capture: true }
		);

		return unsub;
	});
</script>

{#if target}
	<div
		class="focus-cursor"
		style="width: {width}px; height: {height}px; left: {left}px; top: {top}px"
	></div>
{/if}

<style lang="postcss">
	.focus-cursor {
		z-index: 9999;
		position: absolute;

		border: 4px solid blue;
		pointer-events: none;
	}
</style>
