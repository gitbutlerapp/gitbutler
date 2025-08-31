<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		class?: string;
		stayOpen?: boolean;
		thin?: boolean;
		children: Snippet<[thin: boolean]>;
	}

	const { class: className = '', stayOpen, thin = false, children }: Props = $props();
</script>

<div role="group" class="overflow-actions {className}" class:show={stayOpen}>
	{@render children(thin)}
</div>

<style lang="postcss">
	.overflow-actions {
		--show: initial;
		display: flex;

		z-index: var(--z-lifted);
		position: absolute;
		top: -9px;
		right: 10px;
		transform: var(--show, translateY(2px));
		opacity: var(--show, 0);
		pointer-events: var(--show, none);

		transition:
			opacity var(--transition-fast),
			transform var(--transition-medium);

		:global(:first-child .overflow-actions-btn) {
			border-top-left-radius: var(--radius-m);
			border-bottom-left-radius: var(--radius-m);
		}

		:global(:last-child .overflow-actions-btn) {
			border-right: 1px solid var(--clr-border-2);
			border-top-right-radius: var(--radius-m);
			border-bottom-right-radius: var(--radius-m);
		}
	}

	.overflow-actions.show {
		--show: on;
	}
</style>
