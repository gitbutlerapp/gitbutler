<script lang="ts">
	import { UserService } from '$lib/stores/user';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';

	interface Props {
		title?: string;
		message?: string;
	}

	const userService = getContext(UserService);
	const loading = userService.loading;

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
		<Button
			loading={$loading}
			style="pop"
			kind="solid"
			onclick={async () => {
				await userService.login();
			}}>Log in or Sign up</Button
		>
	{/snippet}
</SectionCard>
