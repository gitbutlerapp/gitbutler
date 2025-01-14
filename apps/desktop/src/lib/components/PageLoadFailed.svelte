<script lang="ts">
	import loadErrorSvg from '$lib/assets/empty-state/load-error.svg?raw';
	import Button from '@gitbutler/ui/Button.svelte';

	const { error } = $props();
</script>

<div class="page-load-error">
	<div class="wrapper">
		<div class="image">
			{@html loadErrorSvg}
		</div>
		<h2 class="title text-15 text-body text-bold">Failed to load page</h2>
		<p class="error-message text-12 text-body">
			{error ? error : 'An unknown error occurred'}
		</p>
		<div class="actions">
			<Button
				kind="outline"
				icon="update"
				onclick={() => {
					location.reload();
				}}>Reload view</Button
			>
		</div>
	</div>
</div>

<style lang="postcss">
	.page-load-error {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		width: 100%;

		& .wrapper {
			display: flex;
			flex-direction: column;
			max-width: 320px;
			margin-bottom: 20px;
		}

		& .image {
			width: 120px;
			height: 100px;
			margin-bottom: 24px;
		}

		& .title {
			margin-bottom: 8px;
		}

		& .error-message {
			color: var(--clr-text-2);
			white-space: pre-wrap;
			/* text-wrap: balance; */
			margin-bottom: 12px;
		}

		.actions {
			display: flex;
		}
	}
</style>
