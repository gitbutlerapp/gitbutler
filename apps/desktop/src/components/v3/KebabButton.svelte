<script lang="ts">
	import { TestId } from '$lib/testing/testIds';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';

	type Props = {
		flat: boolean;
		open: boolean;
		contextElement: HTMLElement;
		oncontext?: (position: { x: number; y: number }) => void;
		onclick: (element: HTMLElement) => void;
	};

	const { flat, open, contextElement, onclick, oncontext }: Props = $props();

	let activated = $state(false);
	let buttonElement = $state<HTMLElement>();

	function onMouseEnter() {
		activated = true;
	}

	function onMouseLeave() {
		activated = false;
	}

	function onContextMenu(e: MouseEvent) {
		oncontext?.({ x: e.clientX, y: e.clientY });
		e.preventDefault();
	}

	function onClick(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		onclick?.(e.currentTarget as HTMLElement);
	}

	$effect(() => {
		if (contextElement) {
			contextElement.addEventListener('contextmenu', onContextMenu);
			contextElement.addEventListener('mouseenter', onMouseEnter);
			contextElement.addEventListener('mouseleave', onMouseLeave);
			return () => {
				contextElement.removeEventListener('contextmenu', onContextMenu);
				contextElement.removeEventListener('mouseenter', onMouseEnter);
				contextElement.removeEventListener('mouseleave', onMouseLeave);
			};
		}
	});
</script>

{#if flat}
	<button
		bind:this={buttonElement}
		type="button"
		class="branch-menu-btn"
		class:activated
		class:open
		onclick={onClick}
		data-testid={TestId.KebabMenuButton}
	>
		<Icon name="kebab" />
	</button>
{:else}
	<Button
		testId={TestId.KebabMenuButton}
		size="tag"
		icon="kebab"
		kind="ghost"
		{activated}
		onclick={onClick}
	/>
{/if}

<style lang="postcss">
	.branch-menu-btn {
		display: flex;
		padding: 0 4px;
		color: var(--clr-text-1);
		opacity: 0;

		&.activated {
			opacity: 0.5;
		}

		&.open,
		&:hover {
			opacity: 1;
		}
	}
</style>
