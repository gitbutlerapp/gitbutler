<script lang="ts">
	import LoginButtons from '$components/LoginButtons.svelte';
	import { Icon, Section } from '@gitbutler/ui';

	interface Props {
		title?: string;
		message?: string;
	}

	const {
		title: titleLabel = 'Authorization Required',
		message = 'Authorize GitButler to use this service'
	}: Props = $props();
</script>

<Section>
	<Section.Card>
		{#snippet iconSide()}
			<Icon name="warning" color="warning" />
		{/snippet}
		{#snippet title()}
			{titleLabel}
		{/snippet}
		{#snippet caption()}
			{message}
		{/snippet}
		{#snippet actions()}
			<LoginButtons />
		{/snippet}
	</Section.Card>
</Section>
