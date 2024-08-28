<script lang="ts">
	import AuthorizationBanner from '$lib/settings/AuthorizationBanner.svelte';
	import GithubIntegration from '$lib/settings/GithubIntegration.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '$lib/utils/context';
	import SettingsPage from '@gitbutler/ui/layout/SettingsPage.svelte';

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
