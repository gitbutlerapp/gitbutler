<script lang="ts">
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
</script>

<SettingsPage title="Experimental features">
	<p class="text-13 text-body experimental-settings__text">
		This sections contains a list of feature flags for features that are still in development or in
		an experimental stage.
	</p>
</SettingsPage>

<style>
	.experimental-settings__text {
		color: var(--clr-text-2);
		margin-bottom: 10px;
	}
</style>
