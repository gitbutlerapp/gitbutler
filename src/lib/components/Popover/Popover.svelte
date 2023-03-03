<script lang="ts">
	import { slide } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';

	let showPopover: boolean = true;
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
</script>

<svelte:window on:resize={initPosition} on:keydown={() => (showPopover = false)} />

<div use:clickOutside={() => (showPopover = false)}>
	<button on:click={() => (showPopover = !showPopover)} bind:this={anchor} class="text-zinc-50"
		><slot name="button" /></button
	>

	<!-- svelte-ignore a11y-click-events-have-key-events -->
	{#if showPopover}
		<div
			role="dialog"
			aria-labelledby="Title"
			aria-describedby="Description"
			aria-orientation="vertical"
			transition:slide={{ duration: 150, easing: cubicOut }}
			on:click|stopPropagation
			class="wrapper z-50 bg-zinc-800 border border-zinc-700 text-zinc-50 rounded shadow-2xl min-w-[180px] max-w-[512px]"
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
