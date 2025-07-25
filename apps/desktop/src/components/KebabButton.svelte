<script lang="ts">
	import { TestId } from '$lib/testing/testIds';
	import { Button, Icon } from '@gitbutler/ui';

	type Props = {
		flat?: boolean;
		activated: boolean;
		contextElement: HTMLElement;
		contextElementSelected?: boolean;
		oncontext?: (position: { x: number; y: number }) => void;
		onclick: (element: HTMLElement) => void;
	};

	let { flat, activated, contextElement, contextElementSelected, onclick, oncontext }: Props =
		$props();

	let visible = $state(false);
	let isContextElementFocused = $state(false);
	let buttonElement = $state<HTMLElement>();

	function onMouseEnter() {
		if (!flat) return;
		visible = true;
	}

	function onMouseLeave() {
		if (!flat) return;
		visible = false;
	}

	function onFocus() {
		if (!flat) return;
		isContextElementFocused = true;
		visible = true;
	}
	function onBlur() {
		if (!flat) return;
		isContextElementFocused = false;
		visible = false;
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
			contextElement.addEventListener('focus', onFocus);
			contextElement.addEventListener('blur', onBlur);
			return () => {
				contextElement.removeEventListener('contextmenu', onContextMenu);
				contextElement.removeEventListener('mouseenter', onMouseEnter);
				contextElement.removeEventListener('mouseleave', onMouseLeave);
				contextElement.removeEventListener('focus', onFocus);
				contextElement.removeEventListener('blur', onBlur);
			};
		}
	});
</script>

{#if flat}
	<button
		bind:this={buttonElement}
		type="button"
		class="menu-btn"
		class:visible={visible || isContextElementFocused || contextElementSelected}
		class:activated
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
	.menu-btn {
		display: flex;
		display: none;
		padding: 0 4px;
		color: var(--clr-text-1);

		&.visible {
			display: flex;
			opacity: 0.5;
		}

		&.activated,
		&:hover,
		&:focus-within {
			display: flex;
			opacity: 1;
		}
	}
</style>
