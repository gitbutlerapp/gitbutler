<script lang="ts">
	import AuthorizationBanner from '$components/AuthorizationBanner.svelte';
	import GithubIntegration from '$components/GithubIntegration.svelte';
	import SettingsPage from '$components/SettingsPage.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Spacer from '@gitbutler/ui/Spacer.svelte';

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
