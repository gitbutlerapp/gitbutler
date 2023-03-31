<script lang="ts">
	import { offset, flip, shift } from 'svelte-floating-ui/dom';
	import { arrow } from 'svelte-floating-ui';
	import { createFloatingActions } from 'svelte-floating-ui';
	import { writable } from 'svelte/store';

	let arrowRef = writable({} as HTMLElement);

	export let label: string;

	const [floatingRef, floatingContent, update] = createFloatingActions({
		strategy: 'absolute',
		placement: 'bottom',
		middleware: [offset(8), flip(), shift(), arrow({ element: arrowRef })],
		onComputed({ placement, middlewareData }) {
			if (!middlewareData.arrow) return;
			const { x, y } = middlewareData.arrow;
			const staticSide = {
				top: 'bottom',
				right: 'left',
				bottom: 'top',
				left: 'right'
			}[placement.split('-')[0]] as any;

			if (!$arrowRef) return;
			Object.assign($arrowRef.style, {
				left: x != null ? `${x}px` : '',
				top: y != null ? `${y}px` : '',
				[staticSide]: '-4px'
			});
		}
	});

	let showTooltip = false;
	const timeoutMilliseconds = 1000;
	let timeout: ReturnType<typeof setTimeout>;
</script>

<div
	class="flex-auto overflow-auto"
	on:mouseenter={() => (timeout = setTimeout(() => (showTooltip = true), timeoutMilliseconds))}
	on:mouseleave={() => {
		clearTimeout(timeout);
		showTooltip = false;
	}}
	use:floatingRef
>
	<slot />
</div>

{#if showTooltip}
	<div
		role="tooltip"
		class="
            absolute
            z-[9000]
            rounded-[5px]
            bg-[#171717]
            p-2
            text-[12px]
            text-zinc-300
			shadow-lg
    "
		use:floatingContent
	>
		{label}
		<div
			class="
                absolute
                h-3
                w-3 rotate-45
                rounded-sm
                bg-[#171717]
        "
			bind:this={$arrowRef}
		/>
	</div>
{/if}
