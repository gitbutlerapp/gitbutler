<script lang="ts">
	import Login from '$components/Login.svelte';
	import { Icon, SectionCard } from '@gitbutler/ui';

	interface Props {
		title?: string;
		message?: string;
	}

	const {
		title: titleLabel = 'Authorization Required',
		message = 'Authorize GitButler to use this service'
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
