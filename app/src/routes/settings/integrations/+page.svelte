<script lang="ts">
	import GithubIntegration from '$lib/components/GithubIntegration.svelte';
	import ContentWrapper from '$lib/components/settings/ContentWrapper.svelte';
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
