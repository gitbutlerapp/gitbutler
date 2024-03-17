<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';

	export let title: string;
	export let loading = false;
</script>

<button class="action" class:loading on:click on:mousedown disabled={loading}>
	<div class="icon">
		<!-- prettier-ignore -->
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
		border-radius: var(--radius-l);
		border: 1px solid var(--clr-theme-container-outline-light);
		display: flex;
		position: relative;
		padding: var(--size-16);
		flex-direction: row;
		gap: var(--size-16);
		align-items: center;
		text-align: left;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&:hover {
			background-color: color-mix(in srgb, var(--clr-theme-scale-pop-50) 6%, transparent);
			border: 1px solid color-mix(in srgb, var(--clr-theme-scale-pop-50) 30%, transparent);
		}

		&:focus {
			outline: none;
			background-color: color-mix(in srgb, var(--clr-theme-scale-pop-50) 6%, transparent);
			border: 1px solid color-mix(in srgb, var(--clr-theme-scale-pop-50) 60%, transparent);
		}
	}

	.loading {
		pointer-events: none;
		background-color: color-mix(in srgb, var(--clr-theme-scale-ntrl-50) 8%, transparent);
		border: 1px solid transparent;

		& .action__content {
			opacity: 0.3;
		}
	}

	.action__content {
		position: relative;
		z-index: 0;
		display: flex;
		flex-direction: column;
		gap: var(--size-10);
		transition: opacity var(--transition-slow);
	}

	.action__spinner {
		position: absolute;
		top: var(--size-10);
		right: var(--size-10);
		display: flex;
	}

	.action__title {
		color: var(--clr-theme-scale-ntrl-0);
	}

	.action__message {
		color: var(--clr-theme-scale-ntrl-30);
		max-width: 80%;
	}

	.icon {
		position: relative;
		z-index: 1;
		flex-shrink: 0;
		width: calc(var(--size-40) * 2);
	}
</style>
