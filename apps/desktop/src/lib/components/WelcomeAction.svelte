<script lang="ts">
	import Icon from '$lib/shared/Icon.svelte';
	import type { Snippet } from 'svelte';

	const {
		title,
		loading = false,
		onmousedown,
		onclick,
		icon,
		message
	}: {
		title: string;
		loading: boolean;
		onmousedown?: (e: MouseEvent) => void;
		onclick?: (e: MouseEvent) => void;
		icon: Snippet;
		message: Snippet;
	} = $props();
</script>

<button class="action" class:loading {onclick} {onmousedown} disabled={loading}>
	<div class="action__wrapper">
		<div class="icon">
			{@render icon()}
		</div>
		<div class="action__content">
			<div class="action__title text-base-18 text-bold">{title}</div>
			<div class="action__message text-base-body-12">
				{@render message()}
			</div>
		</div>
		{#if loading}
			<div class="action__spinner">
				<Icon name="spinner" />
			</div>
		{/if}
	</div>
</button>

<style lang="postcss">
	.action {
		container-type: inline-size;
		width: 100%;
	}
	.action__wrapper {
		position: relative;
		display: flex;
		height: 100%;
		overflow: hidden;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		position: relative;
		padding: 16px;
		flex-direction: row;
		gap: 20px;
		align-items: center;
		text-align: left;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);
		background-color: var(--clr-bg-1);

		&:hover,
		&:focus {
			outline: none;
			background-color: var(--clr-bg-1-muted);
		}
	}

	@container (width >= 300px) {
		.action__wrapper {
			flex-direction: row-reverse;
		}
	}

	@container (width <= 300px) {
		.action__wrapper {
			flex-direction: column;
			align-items: start;
		}
	}

	.loading {
		pointer-events: none;
		background-color: var(--clr-bg-2);
		border: 1px solid transparent;
		opacity: 0.5;
	}

	.action__content {
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 10px;
		transition: opacity var(--transition-slow);
	}

	.action__spinner {
		position: absolute;
		top: 10px;
		right: 10px;
		display: flex;
	}

	.action__title {
		color: var(--clr-scale-ntrl-0);
	}

	.action__message {
		color: var(--clr-scale-ntrl-30);
		max-width: 80%;
	}

	.icon {
		opacity: 0.8;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: 80px;
		height: 80px;
		border-radius: var(--radius-m);
		background-color: var(--clr-illustration-bg);
	}
</style>
