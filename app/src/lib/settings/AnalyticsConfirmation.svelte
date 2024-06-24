<script lang="ts">
	import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
	import AnalyticsSettings from '$lib/settings/AnalyticsSettings.svelte';
	import Button from '$lib/shared/Button.svelte';
	import type { Writable } from 'svelte/store';

	export let analyticsConfirmed: Writable<boolean>;
</script>

<div class="analytics-confirmation">
	<h1 class="title text-serif-40">Before we begin</h1>
	<AnalyticsSettings />

	<div class="analytics-confirmation__actions">
		<Button
			style="pop"
			kind="solid"
			icon="chevron-right-small"
			on:click={() => {
				$analyticsConfirmed = true;
				initAnalyticsIfEnabled();
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
		gap: 12px;
	}

	.title {
		color: var(--clr-scale-ntrl-0);
	}

	.analytics-confirmation__actions {
		display: flex;
		justify-content: flex-end;
	}
</style>
