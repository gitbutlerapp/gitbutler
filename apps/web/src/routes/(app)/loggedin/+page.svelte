<script lang="ts">
	import { goto } from '$app/navigation';
	import FullscreenUtilityCard from '$lib/components/service/FullscreenUtilityCard.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { AsyncButton, Button, chipToasts } from '@gitbutler/ui';
	import { copyToClipboard } from '@gitbutler/ui/utils/clipboard';

	const loginService = inject(LOGIN_SERVICE);

	async function copyAccessToken() {
		const response = await loginService.token();
		if (response.type === 'success' && response.data) {
			copyToClipboard(response.data);
		} else {
			chipToasts.error('Failed to get token');
		}
	}
</script>

<svelte:head>
	<title>GitButler | Logged in</title>
</svelte:head>

<FullscreenUtilityCard
	title="Signed in successfully 🎯"
	backlink={{
		label: 'Main',
		href: '/'
	}}
>
	<div class="loggedin__success-card-content">
		<p class="text-13">You can now close this window and return to your client.</p>
		<div class="flex gap-8 m-t-8">
			<AsyncButton style="gray" kind="outline" icon="copy-small" action={copyAccessToken}
				>Copy Access Token</AsyncButton
			>
			<Button style="gray" kind="ghost" onclick={() => goto('/profile')} icon="profile"
				>Profile page</Button
			>
		</div>
	</div>
</FullscreenUtilityCard>

<style>
	.loggedin__success-card-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
</style>
