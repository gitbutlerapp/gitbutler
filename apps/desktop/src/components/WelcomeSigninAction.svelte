<script lang="ts">
	import signinSvg from '$lib/assets/signin.svg?raw';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { Button, CardGroup, Textbox } from '@gitbutler/ui';

	const userService = inject(USER_SERVICE);
	const user = userService.user;

	let accessToken = $state('');
</script>

{#if !$user}
	<CardGroup>
		<CardGroup.Item>
			{#snippet title()}
				<h3 class="text-18 text-bold">Access Token</h3>
			{/snippet}
			<p class="text-12 text-body clr-text-2">
				An access token is required to use GitButler's smart automation features, including
				intelligent branch creation and commit message generation.
			</p>
			<Textbox
				size="large"
				type="password"
				value={accessToken}
				placeholder="************************"
				oninput={(value) => (accessToken = value)}
			/>
			<Button
				style="pop"
				disabled={accessToken.trim().length === 0}
				onclick={async () => {
					await userService.setUserAccessToken(accessToken.trim());
					accessToken = '';
				}}>Save Access Token</Button
			>
		</CardGroup.Item>
		<CardGroup.Item>
			{#snippet title()}
				<h3 class="text-18 text-bold">Get an Access Token</h3>
			{/snippet}
			{#snippet caption()}
				<p class="text-12 text-body clr-text-2">
					No Access Token? No problem! Sign up or log in to GitButler to generate your personal
					access token and unlock
				</p>
				<div class="flex gap-8 m-t-8">
					<Button
						style="pop"
						onclick={async () => {
							await userService.openLoginPage();
						}}>Log in / Sign up</Button
					>

					<Button
						kind="outline"
						icon="copy-small"
						onclick={async () => {
							await userService.copyLoginPageLink();
						}}>Copy login link</Button
					>
				</div>
			{/snippet}
			{#snippet actions()}
				<div class="signin-svg">
					{@html signinSvg}
				</div>
			{/snippet}
		</CardGroup.Item>
	</CardGroup>
{/if}

<style lang="postcss">
	.signin-svg {
		flex-shrink: 0;
		width: 100px;
		height: 70px;
		border-radius: var(--radius-m);
		background-color: var(--clr-art-scene-bg);
	}
</style>
