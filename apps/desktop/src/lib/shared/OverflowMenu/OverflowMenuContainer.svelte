<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		class?: string;
		isOpen?: boolean;
		children: Snippet;
	}

	const { class: className = '', isOpen, children }: Props = $props();
</script>

<div class="overflow-actions {className}" class:show={isOpen}>
	{@render children()}
</div>

<style lang="postcss">
	.overflow-actions {
		--show: initial;
		display: flex;
		position: absolute;
		top: -9px;
		right: 14px;
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		overflow: hidden;
		/* animated props */
		pointer-events: var(--show, none);
		opacity: var(--show, 0);
		transform: var(--show, translateY(2px));

		transition:
			opacity var(--transition-fast),
			transform var(--transition-medium);

		:global(span:first-child .header-menu__btn) {
			border-left: none;
		}
	}

	.overflow-actions.show {
		--show: on;
	}
</style>
