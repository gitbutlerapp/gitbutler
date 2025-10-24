<script lang="ts">
	import { goto } from '$app/navigation';
	import UserAuthAvatar from '$lib/components/UserAuthAvatar.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button } from '@gitbutler/ui';

	interface Props {
		hideIfUserAuthenticated?: boolean;
	}

	const { hideIfUserAuthenticated = false }: Props = $props();

	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);
	const routes = inject(WEB_ROUTES_SERVICE);
</script>

{#if $user && !hideIfUserAuthenticated}
	<UserAuthAvatar user={$user} />
{:else if !$user}
	<div class="login-signup-wrap">
		<Button kind="outline" onclick={() => goto(routes.signupPath())}>Join GitButler</Button>
		<Button style="pop" onclick={() => goto(routes.loginPath())} icon="signin">Log in</Button>
	</div>
{/if}

<style lang="postcss">
	.login-signup-wrap {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>
