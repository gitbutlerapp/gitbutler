<script lang="ts">
	import type { ComponentType } from 'svelte';
	import { IconLoading } from '../icons';

	export let role: 'basic' | 'primary' | 'destructive' = 'basic';
	export let filled = true;
	const outlined = true;
	export let disabled = false;
	export let height: 'basic' | 'small' = 'basic';
	export let width: 'basic' | 'long' = 'basic';
	export let type: 'button' | 'submit' = 'button';
	export let href: string | undefined = undefined;
	export let icon: ComponentType | undefined = undefined;
	export let loading = false;
</script>

{#if href}
	<a
		{href}
		class="relative cursor-pointer {role} flex w-fit justify-center gap-[10px] whitespace-nowrap rounded border text-base font-medium text-zinc-50 transition ease-in-out"
		class:small={height === 'small'}
		class:long={width === 'long'}
		class:filled
		class:pointer-events-none={loading}
		class:outlined
		{type}
		class:disabled
	>
		{#if loading}
			{#if icon}
				<IconLoading class="h-[16px] w-[16px] animate-spin" />
				<slot />
			{:else}
				<div class="items-around absolute flex h-full w-full justify-center">
					<IconLoading class="h-[16px] w-[16px] animate-spin" />
				</div>
				<div class="opacity-0">
					<slot />
				</div>
			{/if}
		{:else}
			<svelte:component this={icon} class="h-[16px] w-[16px]" />
			<slot />
		{/if}
	</a>
{:else}
	<button
		class="relative cursor-pointer {role} flex w-fit justify-center gap-[10px] whitespace-nowrap rounded border text-base font-medium text-zinc-50 transition ease-in-out"
		class:small={height === 'small'}
		class:long={width === 'long'}
		class:pointer-events-none={loading}
		class:filled
		class:outlined
		{disabled}
		{type}
		class:disabled
		on:click
	>
		{#if loading}
			{#if icon}
				<IconLoading class="h-[16px] w-[16px] animate-spin" />
				<slot />
			{:else}
				<div class="items-around absolute flex h-full w-full justify-center">
					<IconLoading class="h-[16px] w-[16px] animate-spin" />
				</div>
				<div class="opacity-0">
					<slot />
				</div>
			{/if}
		{:else}
			<svelte:component this={icon} class="h-[16px] w-[16px]" />
			<slot />
		{/if}
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
