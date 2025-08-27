<script lang="ts">
	import Login from '$components/Login.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { Icon, SectionCard } from '@gitbutler/ui';

	interface Props {
		title?: string;
		message?: string;
	}

	const {
		title: titleLabel = 'Authorization Required',
		message = 'You need to authorize GitButler to access this service.'
	}: Props = $props();
</script>

<SectionCard orientation="row" {focusable}>
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
