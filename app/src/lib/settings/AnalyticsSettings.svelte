<script lang="ts">
	import Link from '$lib/components/Link.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import {
		appErrorReportingEnabled,
		appMetricsEnabled,
		appNonAnonMetricsEnabled
	} from '$lib/config/appSettings';

	const errorReportingEnabled = appErrorReportingEnabled();
	const metricsEnabled = appMetricsEnabled();
	const nonAnonMetricsEnabled = appNonAnonMetricsEnabled();
</script>

<section class="analytics-settings">
	<div class="analytics-settings__content">
		<p class="text-base-body-13 analytics-settings__text">
			GitButler uses telemetry strictly to help us improve the client. We do not collect any
			personal information (<Link
				target="_blank"
				rel="noreferrer"
				href="https://gitbutler.com/privacy"
			>
				privacy policy
			</Link>).
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
		<SectionCard labelFor="errorReportingToggle" orientation="row">
			<svelte:fragment slot="title">Error reporting</svelte:fragment>
			<svelte:fragment slot="caption">
				Toggle reporting of application crashes and errors.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="errorReportingToggle"
					checked={$errorReportingEnabled}
					on:click={() => ($errorReportingEnabled = !$errorReportingEnabled)}
				/>
			</svelte:fragment>
		</SectionCard>

		<SectionCard labelFor="metricsEnabledToggle" orientation="row">
			<svelte:fragment slot="title">Usage metrics</svelte:fragment>
			<svelte:fragment slot="caption">Toggle sharing of usage statistics.</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="metricsEnabledToggle"
					checked={$metricsEnabled}
					on:click={() => ($metricsEnabled = !$metricsEnabled)}
				/>
			</svelte:fragment>
		</SectionCard>

		<SectionCard labelFor="nonAnonMetricsEnabledToggle" orientation="row">
			<svelte:fragment slot="title">Non-anonymous usage metrics</svelte:fragment>
			<svelte:fragment slot="caption">
				Toggle sharing of identifiable usage statistics.
			</svelte:fragment>
			<svelte:fragment slot="actions">
				<Toggle
					id="nonAnonMetricsEnabledToggle"
					checked={$nonAnonMetricsEnabled}
					on:click={() => ($nonAnonMetricsEnabled = !$nonAnonMetricsEnabled)}
				/>
			</svelte:fragment>
		</SectionCard>
	</div>
</section>

<style lang="postcss">
	.analytics-settings {
		display: flex;
		flex-direction: column;
		gap: 28px;
	}

	.analytics-settings__content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.analytics-settings__text {
		color: var(--clr-text-2);
	}

	.analytics-settings__actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
