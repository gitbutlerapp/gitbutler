<script lang="ts">
	import AuthorizationBanner from '$components/shared/AuthorizationBanner.svelte';
	import GithubIntegration from '$components/shared/GithubIntegration.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import { Spacer } from '@gitbutler/ui';

	const userService = inject(USER_SERVICE);
	const user = userService.user;
</script>

{#if !$user}
	<AuthorizationBanner />
	<Spacer />
{/if}
<GithubIntegration disabled={!$user} />
