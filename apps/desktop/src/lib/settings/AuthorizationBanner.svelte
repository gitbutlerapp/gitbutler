<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';

	interface Props {
		title?: string;
		message?: string;
	}

	const userService = getContext(UserService);
	const loading = userService.loading;

	const {
		title = 'Authorization Required',
		message = 'You need to authorize GitButler to access this service.'
	}: Props = $props();
</script>

<SectionCard orientation="row">
	<svelte:fragment slot="iconSide">
		<Icon name="warning" color="warning" />
	</svelte:fragment>
	<svelte:fragment slot="title">
		{title}
	</svelte:fragment>
	<svelte:fragment slot="caption">
		{message}
	</svelte:fragment>
	<svelte:fragment slot="actions">
		<Button
			loading={$loading}
			style="pop"
			kind="solid"
			onclick={async () => {
				await userService.login();
			}}>Log in or Sign up</Button
		>
	</svelte:fragment>
</SectionCard>
