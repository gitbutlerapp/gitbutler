<script lang="ts">
	import { offset, flip, shift } from 'svelte-floating-ui/dom';
	import { createFloatingActions } from 'svelte-floating-ui';

	export let label: string | undefined = undefined;
	type Placement = 'top' | 'right' | 'bottom' | 'left';
	export let placement: Placement = 'bottom';
	export let timeoutMilliseconds = 1000;

	const [floatingRef, floatingContent] = createFloatingActions({
		strategy: 'absolute',
		placement: placement,
		middleware: [offset(4), flip(), shift()]
	});

	let showTooltip = false;
	let timeout: ReturnType<typeof setTimeout>;
</script>

<div
	role="tooltip"
	class="wrapper"
	on:mouseenter={() => (timeout = setTimeout(() => (showTooltip = true), timeoutMilliseconds))}
	on:mouseleave={() => {
		clearTimeout(timeout);
		showTooltip = false;
	}}
	use:floatingRef
>
	<slot />
	{#if showTooltip}
		<div role="tooltip" class="tooltip text-base-11" use:floatingContent>
			<slot name="label" />
			{#if label}
				{label}
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	.wrapper {
		display: inline-block;
	}
	.tooltip {
		background-color: var(--clr-core-ntrl-10);
		border-radius: var(--radius-s);
		border: 1px solid var(--clr-core-ntrl-30);
		color: var(--clr-core-ntrl-60);
		display: inline-block;
		padding-left: var(--space-8);
		padding-right: var(--space-8);
		padding-top: var(--space-4);
		padding-bottom: var(--space-4);
		z-index: 50;
	}
</style>
