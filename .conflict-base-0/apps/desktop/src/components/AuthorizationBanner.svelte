<script lang="ts">
	import Login from '$components/Login.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';

	interface Props {
		title?: string;
		message?: string;
	}

	const {
		title: titleLabel = 'Authorization Required',
		message = 'You need to authorize GitButler to access this service.'
	}: Props = $props();
</script>

<SectionCard orientation="row">
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
		<Login />
	{/snippet}
</SectionCard>
