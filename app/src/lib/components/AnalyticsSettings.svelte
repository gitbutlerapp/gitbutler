<script lang="ts">
	import Link from './Link.svelte';
	import SectionCard from './SectionCard.svelte';
	import Toggle from './Toggle.svelte';
	import { appErrorReportingEnabled, appMetricsEnabled } from '$lib/config/appSettings';

	const errorReportingEnabled = appErrorReportingEnabled();
	const metricsEnabled = appMetricsEnabled();

	function toggleErrorReporting() {
		$errorReportingEnabled = !$errorReportingEnabled;
	}

	function toggleMetrics() {
		$metricsEnabled = !$metricsEnabled;
	}
</script>

<section class="analytics-settings">
	<div class="analytics-settings__content">
		<p class="text-base-body-13 analytics-settings__text">
			GitButler uses telemetry strictly to help us improve the client. We do not collect any
			personal information.
		</p>
		<p class="text-base-body-13 analytics-settings__text">
			We kindly ask you to consider keeping these settings enabled as it helps us catch issues more
			quickly. If you choose to disable them, please feel to share your feedback on our <Link
				target="_blank"
				rel="noreferrer"
				href="https://discord.gg/MmFkmaJ42D"
			>
				Discord
			</Link>.
		</p>
	</div>

	<div class="analytics-settings__actions">
		<SectionCard labelFor="errorReportngToggle" on:click={toggleErrorReporting} orientation="row">
			<svelte:fragment slot="title">Error reporting</svelte:fragment>
			<svelte:fragment slot="caption">
				Toggle reporting of application crashes and errors.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="errorReportngToggle"
					checked={$errorReportingEnabled}
					on:change={toggleErrorReporting}
				/>
			</svelte:fragment>
		</SectionCard>

		<SectionCard labelFor="metricsEnabledToggle" on:click={toggleMetrics} orientation="row">
			<svelte:fragment slot="title">Usage metrics</svelte:fragment>
			<svelte:fragment slot="caption">Toggle sharing of usage statistics.</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle id="metricsEnabledToggle" checked={$metricsEnabled} on:change={toggleMetrics} />
			</svelte:fragment>
		</SectionCard>
	</div>
</section>

<style lang="postcss">
	.analytics-settings {
		display: flex;
		flex-direction: column;
		gap: var(--size-28);
	}

	.analytics-settings__content {
		display: flex;
		flex-direction: column;
		gap: var(--size-16);
	}

	.analytics-settings__text {
		color: var(--clr-text-2);
	}

	.analytics-settings__actions {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
	}
</style>
