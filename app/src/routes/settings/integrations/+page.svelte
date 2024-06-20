<script lang="ts">
	import ContentWrapper from '$lib/settings/ContentWrapper.svelte';
	import GithubIntegration from '$lib/settings/GithubIntegration.svelte';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '$lib/utils/context';

	const userService = getContext(UserService);
	const user = userService.user;
</script>

<ContentWrapper title="Integrations">
	{#if $user}
		<GithubIntegration />
	{/if}
</ContentWrapper>
