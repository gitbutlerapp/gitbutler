<script lang="ts">
	import { onMount, type ComponentType } from 'svelte';
	import { IconLoading } from '../icons';

	export let role: 'basic' | 'primary' | 'destructive' | 'purple' = 'basic';
	export let filled = true;
	export let outlined = false;
	export let disabled = false;
	export let height: 'basic' | 'small' = 'basic';
	export let width: 'basic' | 'full-width' = 'basic';
	export let type: 'button' | 'submit' = 'button';
	export let icon: ComponentType | undefined = undefined;
	export let loading = false;

	let element: HTMLAnchorElement | HTMLButtonElement;

	onMount(() => {
		element.ariaLabel = element.innerText.trim();
	});
</script>

<button
	class={role}
	class:small={height === 'small'}
	class:full-width={width === 'full-width'}
	class:pointer-events-none={loading}
	bind:this={element}
	class:filled
	class:outlined
	{disabled}
	{type}
	class:disabled
	on:click
	class:px-4={!!$$slots.default}
	class:px-2={!$$slots.default}
>
	{#if loading}
		{#if icon}
			<IconLoading class="h-4 w-4 animate-spin" />
			<slot />
		{:else}
			<div class="items-around absolute flex w-full justify-center">
				<IconLoading class="h-4 w-4 animate-spin" />
			</div>
			<div class="opacity-0">
				<slot />
			</div>
		{/if}
	{:else}
		<svelte:component this={icon} class="h-4 w-4" />
		<slot />
	{/if}
</button>

<style lang="postcss">
	button {
		@apply relative flex h-[36px] w-fit cursor-pointer items-center justify-center gap-[10px] whitespace-nowrap rounded py-2 text-base font-medium transition transition duration-150 ease-in-out ease-out hover:ease-in;
	}

	button:focus {
		@apply outline-none;
	}

	.basic {
		@apply text-zinc-300;
	}
	.basic:hover {
		@apply bg-[#D4D4D8]/20;
	}
	.basic.outlined {
		@apply border-zinc-500;
	}
	.basic.outlined:hover {
		@apply bg-[#FFFFFF1A]/10;
	}
	.basic.filled {
		@apply bg-zinc-500;
	}
	.basic.filled:hover {
		@apply bg-zinc-600;
	}

	.primary {
		@apply text-blue-500;
	}
	.primary:hover {
		@apply bg-[#3B82F6]/20;
	}
	.primary.outlined {
		@apply border-[#3662E3];
	}
	.primary.outlined:hover {
		@apply bg-[#1C48C94D]/20;
	}
	.primary.filled {
		@apply bg-blue-600;
	}
	.primary.filled:hover {
		@apply bg-[#1C48C9];
	}

	.destructive {
		@apply text-red-600;
	}
	.destructive:hover {
		@apply bg-[#DC2626]/20;
	}
	.destructive.outlined {
		@apply border-[#E33636];
	}
	.destructive.outlined:hover {
		@apply bg-[#E336364D]/30;
	}
	.destructive.filled {
		@apply bg-[#E33636];
	}
	.destructive.filled:hover {
		@apply bg-[#C91C1C];
	}

	.purple {
		@apply text-[#5852A0];
	}
	.purple.outlined {
		@apply border-[#524C93];
	}
	.purple.outlined:hover {
		@apply bg-[#524C93]/20;
	}
	.purple.filled {
		@apply bg-[#5852A0];
	}
	.purple.filled:hover {
		@apply bg-[#423E7A];
	}

	.disabled {
		@apply pointer-events-none text-zinc-500;
	}

	.filled.disabled,
	.outlined.disabled {
		@apply opacity-40;
	}

	.filled {
		border-top: 1px solid rgba(255, 255, 255, 0.2);
		border-bottom: 1px solid rgba(0, 0, 0, 0.3);
		border-left: 1px solid rgba(255, 255, 255, 0);
		border-right: 1px solid rgba(255, 255, 255, 0);
		text-shadow: 0px 2px #00000021;
	}

	.filled,
	.outlined {
		@apply text-zinc-50;
	}

	.outlined {
		@apply border;
	}

	.small {
		@apply h-[24px] py-[1px];
	}

	.full-width {
		@apply w-full;
	}
</style>
