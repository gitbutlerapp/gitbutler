<script lang="ts">
	import type { ComponentType } from 'svelte';
	import { IconLoading } from '../icons';

	export let role: 'basic' | 'primary' | 'destructive' = 'basic';
	export let filled = true;
	export let outlined = false;
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
		class={role}
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
		class={role}
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
	a,
	button {
		@apply relative flex w-fit cursor-pointer justify-center gap-[10px] whitespace-nowrap rounded text-base font-medium transition ease-in-out hover:underline;
		text-underline-offset: 3px;
	}

	.basic {
		@apply text-zinc-300;
	}
	.basic.outlined {
		@apply border-zinc-500;
	}
	.basic.outlined:hover {
		@apply bg-[#FFFFFF1A]/10;
	}
	.basic.filled {
		@apply border-transparent bg-zinc-500;
	}
	.basic.filled:hover {
		@apply bg-zinc-600;
	}

	.primary {
		@apply text-blue-500;
	}
	.primary.outlined {
		@apply border-[#3662E3];
	}
	.primary.outlined:hover {
		@apply bg-[#1C48C94D]/30;
	}
	.primary.filled {
		@apply border-transparent bg-blue-600;
	}
	.primary.filled:hover {
		@apply bg-[#1C48C9];
	}

	.destructive {
		@apply text-red-600;
	}
	.destructive.outlined {
		@apply border-[#E33636];
	}
	.destructive.outlined:hover {
		@apply bg-[#E336364D]/30;
	}
	.destructive.filled {
		@apply border-transparent bg-[#E33636];
	}
	.destructive.filled:hover {
		@apply bg-[#C91C1C];
	}

	.disabled {
		@apply pointer-events-none text-zinc-500;
	}

	.filled.disabled,
	.outlined.disabled {
		@apply opacity-40;
	}

	.filled,
	.outlined {
		@apply px-[16px] py-[10px] text-zinc-50 hover:no-underline;
	}

	.outlined {
		@apply border;
	}

	.filled.small,
	.outlined.small {
		@apply py-[4px];
	}

	.filled.long,
	.outlined.long {
		@apply px-[42px];
	}
</style>
