<script lang="ts">
	import type { ComponentType } from 'svelte';

	export let role: 'basic' | 'primary' | 'destructive' = 'basic';
	export let filled = true;
	const outlined = true;
	export let disabled = false;
	export let height: 'basic' | 'small' = 'basic';
	export let width: 'basic' | 'long' = 'basic';
	export let type: 'button' | 'submit' = 'button';
	export let href: string | undefined = undefined;
	export let icon: ComponentType | undefined = undefined;
</script>

{#if href}
	<a
		{href}
		class="{role} flex w-fit justify-center gap-[10px] whitespace-nowrap rounded border text-base font-medium text-zinc-50 transition ease-in-out"
		class:small={height === 'small'}
		class:long={width === 'long'}
		class:filled
		class:outlined
		{type}
		on:click
		class:disabled
	>
		<svelte:component this={icon} class="h-[16px] w-[16px]" />
		<slot />
	</a>
{:else}
	<button
		class="{role} flex w-fit justify-center gap-[10px] whitespace-nowrap rounded border text-base font-medium text-zinc-50 transition ease-in-out"
		class:small={height === 'small'}
		class:long={width === 'long'}
		class:filled
		class:outlined
		{disabled}
		{type}
		on:click
		class:disabled
	>
		<svelte:component this={icon} class="h-[16px] w-[16px]" />
		<slot />
	</button>
{/if}

<style lang="postcss">
	.disabled {
		@apply pointer-events-none opacity-40;
	}

	.filled,
	.outlined {
		@apply px-[16px] py-[10px];
	}

	.filled.small,
	.outlined.small {
		@apply py-[4px];
	}

	.filled.long,
	.outlined.long {
		@apply px-[42px];
	}

	.basic {
		@apply border-zinc-500;
	}
	.basic:hover {
		@apply bg-[#FFFFFF1A]/10;
	}
	.basic.filled {
		@apply border-transparent bg-zinc-500;
	}
	.basic.filled:hover {
		@apply bg-zinc-600;
	}

	.primary {
		@apply border-[#3662E3];
	}
	.primary:hover {
		@apply bg-[#1C48C94D]/30;
	}
	.primary.filled {
		@apply border-transparent bg-blue-600;
	}
	.primary.filled:hover {
		@apply bg-[#1C48C9];
	}

	.destructive {
		@apply border-[#E33636];
	}
	.destructive:hover {
		@apply bg-[#E336364D]/30;
	}
	.destructive.filled {
		@apply border-transparent bg-[#E33636];
	}
	.destructive.filled:hover {
		@apply bg-[#C91C1C];
	}
</style>
