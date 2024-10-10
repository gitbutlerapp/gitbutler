<script lang="ts">
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import AuthorizationBanner from '$lib/settings/AuthorizationBanner.svelte';
	import GithubIntegration from '$lib/settings/GithubIntegration.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '@gitbutler/shared/context';

	const userService = getContext(UserService);
	const user = userService.user;
</script>

<SettingsPage title="Integrations">
	{#if !$user}
		<AuthorizationBanner />
		<Spacer />
	{/if}
	<GithubIntegration disabled={!$user} />
</SettingsPage>
