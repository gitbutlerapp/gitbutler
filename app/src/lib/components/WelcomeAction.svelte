<script lang="ts">
	import Icon from '$lib/shared/Icon.svelte';

	export let title: string;
	export let loading = false;
</script>

<button class="action" class:loading on:click on:mousedown disabled={loading}>
	<div class="icon">
		<slot name="icon" />
	</div>
	<div class="action__content">
		<div class="action__title text-base-18 text-bold">{title}</div>
		<div class="action__message text-base-body-12">
			<slot name="message" />
		</div>
	</div>
	{#if loading}
		<div class="action__spinner">
			<Icon name="spinner" />
		</div>
	{/if}
</button>

<style lang="postcss">
	.action {
		position: relative;
		overflow: hidden;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		display: flex;
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
			background-color: oklch(from var(--clr-bg-2) l c h / 0.5);
		}
	}

	.loading {
		pointer-events: none;
		background-color: var(--clr-bg-2);
		border: 1px solid transparent;
		opacity: 0.5;
	}

	.action__content {
		flex: 1;
		position: relative;
		z-index: 0;
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
