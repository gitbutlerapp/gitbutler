<script lang="ts">
	import Button from '$lib/Button.svelte';
	import { cubicOut } from 'svelte/easing';
	import { scale } from 'svelte/transition';

	interface Props {
		hasUnread: boolean;
		onclick: (event?: any) => void;
	}

	const { hasUnread, onclick }: Props = $props();
</script>

<div class="bell-btn">
	{#if hasUnread}
		<div transition:scale={{ duration: 200, easing: cubicOut }} class="bell-btn__indication"></div>
	{/if}

	<Button
		type="button"
		kind="ghost"
		icon="bell"
		{onclick}
		iconClass={hasUnread ? 'bell-shake' : ''}
	/>
</div>

<style lang="postcss">
	.bell-btn {
		position: relative;
		display: flex;
		width: fit-content;
	}

	.bell-btn__indication {
		z-index: var(--z-ground);
		position: absolute;
		top: 3px;
		right: 5px;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background-color: var(--clr-theme-err-element);
		box-shadow: 0 0 0 2px var(--clr-bg-2);
	}

	/* global button styles */
	:global(.bell-btn .bell-shake) {
		animation: shake 0.4s 2 ease-in-out forwards;
	}

	@keyframes shake {
		0% {
			transform: rotate(0deg);
		}
		25% {
			transform: rotate(10deg);
		}
		50% {
			transform: rotate(-10deg);
		}
		75% {
			transform: rotate(5deg);
		}
		100% {
			transform: rotate(0deg);
		}
	}
</style>
