<script lang="ts">
	import Icon from '$lib/icons/Icon.svelte';

	export let title: string;
	export let loading = false;
</script>

<button class="card action" class:loading on:click>
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
		display: flex;
		position: relative;
		padding: var(--space-16);
		flex-direction: row;
		gap: var(--space-16);
		align-items: center;
		text-align: left;
		&:not(.loading):hover {
			background-color: var(--clr-theme-container-pale);
		}
	}

	.loading {
		background-color: var(--clr-theme-container-sub);
	}

	.action__content {
		display: flex;
		flex-direction: column;
		gap: var(--space-10);
	}

	.action__spinner {
		position: absolute;
		top: var(--space-6);
		right: var(--space-6);
	}

	.icon {
		flex-shrink: 0;
	}
</style>
