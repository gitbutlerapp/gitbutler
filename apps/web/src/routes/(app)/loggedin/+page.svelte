<script lang="ts">
	import FullscreenUtilityCard from '$lib/components/service/FullscreenUtilityCard.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { AsyncButton, chipToasts } from '@gitbutler/ui';
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
		<p class="text-14 clr-text-1">
			<span class="loggedin__emphasize">NOW:</span> Copy the access token and paste it in your client
		</p>
		<AsyncButton style="pop" icon="copy" action={copyAccessToken}>Copy Access Token</AsyncButton>
	</div>
</FullscreenUtilityCard>

<style>
	.loggedin__success-card-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		margin-top: 24px;
		gap: 16px;
	}

	.loggedin__emphasize {
		font-weight: 600;
	}
</style>
