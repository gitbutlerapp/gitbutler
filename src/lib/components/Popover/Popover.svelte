<script lang="ts">
	import { fade } from 'svelte/transition';

	let showPopover = false;
	let anchor: HTMLButtonElement | undefined = undefined;
	let bottom: number;
	let left: number;

	const initPosition = () =>
		({ bottom, left } = anchor?.getBoundingClientRect() ?? { bottom: 0, left: 0 });

	$: anchor, initPosition();

	function clickOutside(element: HTMLElement, callback: () => void) {
		const handleClick = (event: Event) => {
			const target = event.target as HTMLElement;
			if (!target) {
				return;
			}
			if (!element.contains(target)) {
				callback();
			}
		};

		document.body.addEventListener('click', handleClick);

		return {
			update(newCallback: () => void) {
				callback = newCallback;
			},
			destroy() {
				document.removeEventListener('click', handleClick, true);
			}
		};
	}

	function fadeAndZoomIn(node: HTMLElement, { delay = 0, duration = 150 }) {
		const o = +getComputedStyle(node).opacity;

		return {
			delay,
			duration,
			css: (t: number) => `
				opacity: ${t * o};
				transform: scale(${t});
				transform-origin: 25% 0%;
			`
		};
	}
</script>

<svelte:window on:resize={initPosition} on:keydown={() => (showPopover = false)} />

<div use:clickOutside={() => (showPopover = false)}>
	<button on:click={() => (showPopover = !showPopover)} bind:this={anchor} class="text-zinc-50"
		><slot name="button" /></button
	>

	{#if showPopover}
		<div
			role="dialog"
			aria-labelledby="Title"
			aria-describedby="Description"
			aria-orientation="vertical"
			in:fadeAndZoomIn={{ duration: 150 }}
			out:fade={{ duration: 100 }}
			on:mouseup={() => (showPopover = false)}
			class="wrapper z-[999] min-w-[180px] max-w-[512px] rounded border border-zinc-700 bg-zinc-800 text-zinc-50 shadow-2xl"
			style="--popover-top: {`${bottom}px`}; --popover-left: {`${left}px`}"
		>
			<slot />
		</div>
	{/if}
</div>

<style>
	.wrapper {
		position: absolute;
		top: calc(var(--popover-top) + 4px);
		left: var(--popover-left);
	}
</style>
