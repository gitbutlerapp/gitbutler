<script lang="ts">
	import Link from './Link.svelte';
	import Toggle from './Toggle.svelte';
	import { appErrorReportingEnabled, appMetricsEnabled } from '$lib/config/appSettings';

	const errorReportingEnabled = appErrorReportingEnabled();
	const metricsEnabled = appMetricsEnabled();
	let updatedTelemetrySettings = false;
</script>

<div class="analytics-settings">
	<div>
		<h2 class="mb-2 text-lg font-medium">Telemetry</h2>
	</div>
	<div class="flex flex-col gap-2">
		<p class="text-sm text-light-700 dark:text-dark-200">
			GitButler uses telemetry strictly to help us improve the client. We do not collect any
			personal information.
		</p>
		<p class="text-sm text-light-700 dark:text-dark-200">
			We kindly ask you to consider keeping these settings enabled as it helps us catch issues more
			quickly. If you choose to disable them, please feel to share your feedback on our <Link
				target="_blank"
				rel="noreferrer"
				href="https://discord.gg/MmFkmaJ42D"
			>
				Discord
			</Link>.
		</p>

		{#if updatedTelemetrySettings}
			<p class="text-sm text-red-500">Changes will take effect on the next application start</p>
		{/if}
	</div>
	<div class="flex items-center">
		<div class="flex-grow">
			<p>Error reporting</p>
			<p class="text-sm text-light-700 dark:text-dark-200">
				Toggle reporting of application crashes and errors.
			</p>
		</div>
		<div>
			<Toggle
				checked={$errorReportingEnabled}
				on:change={() => {
					$errorReportingEnabled = !$errorReportingEnabled;
					updatedTelemetrySettings = true;
				}}
			/>
		</div>
	</div>
	<div class="flex items-center">
		<div class="flex-grow">
			<p>Usage metrics</p>
			<p class="text-sm text-light-700 dark:text-dark-200">Toggle sharing of usage statistics.</p>
		</div>
		<div>
			<Toggle
				checked={$metricsEnabled}
				on:change={() => {
					$metricsEnabled = !$metricsEnabled;
					updatedTelemetrySettings = true;
				}}
			/>
		</div>
	</div>
</div>

<style lang="postcss">
	.analytics-settings {
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
	}
</style>
