<script lang="ts">
	import AnalyticsSettings from './AnalyticsSettings.svelte';
	import Button from './Button.svelte';
	import type { Writable } from 'svelte/store';

	export let analyticsConfirmed: Writable<boolean>;
</script>

<div class="analytics-confirmation">
	<h1 class="title text-serif-40">Before we begin</h1>
	<AnalyticsSettings />

	<div class="analytics-confirmation__actions">
		<Button
			icon="chevron-right-small"
			on:click={() => {
				$analyticsConfirmed = true;
			}}
		>
			Continue
		</Button>
	</div>
</div>

<style lang="postcss">
	.analytics-confirmation {
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: var(--size-12);
	}

	.title {
		color: var(--clr-theme-scale-ntrl-0);
	}

	.analytics-confirmation__actions {
		display: flex;
		justify-content: flex-end;
	}
</style>
