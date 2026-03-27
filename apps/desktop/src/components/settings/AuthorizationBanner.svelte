<script lang="ts">
	import GoToSettingsButton from "$components/settings/GoToSettingsButton.svelte";
	import { CardGroup, Icon } from "@gitbutler/ui";

	interface Props {
		title?: string;
		message?: string;
	}

	const {
		title: titleLabel = "Authorization Required",
		message = "Authorize GitButler to use this service",
	}: Props = $props();
</script>

<CardGroup>
	<CardGroup.Item>
		{#snippet iconSide()}
			<Icon name="warning" color="var(--fill-warn-bg)" />
		{/snippet}
		{#snippet title()}
			{titleLabel}
		{/snippet}
		{#snippet caption()}
			{message}
		{/snippet}
		{#snippet actions()}
			<GoToSettingsButton />
		{/snippet}
	</CardGroup.Item>
</CardGroup>
