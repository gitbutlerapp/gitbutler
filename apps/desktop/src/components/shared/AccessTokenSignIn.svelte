<script lang="ts">
	import signinSvg from "$lib/assets/token.svg?raw";
	import { USER_SERVICE } from "$lib/user/userService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { CardGroup, Textbox, Spacer, AsyncButton } from "@gitbutler/ui";

	const userService = inject(USER_SERVICE);

	let accessToken = $state("");
</script>

{#if !userService.user}
	<CardGroup.Item standalone>
		<div class="token-content">
			<div class="token-svg">
				{@html signinSvg}
			</div>

			<div class="info-section">
				<h2 class="text-15 text-bold m-b-6">Access token</h2>

				<p class="text-12 text-body clr-text-2">
					Sign in to GitButler to get your personal access token.
				</p>

				<div class="flex gap-8 m-t-12">
					<AsyncButton
						style="pop"
						icon="login"
						action={async () => {
							await userService.openLoginPage();
						}}
					>
						Log in / Sign up
					</AsyncButton>

					<AsyncButton
						kind="outline"
						icon="copy"
						action={async () => {
							await userService.copyLoginPageLink();
						}}
					>
						Copy login link
					</AsyncButton>
				</div>

				<Spacer dotted margin={16} />

				<div class="token-fields">
					<Textbox
						size="large"
						type="password"
						value={accessToken}
						placeholder="•••••••••••••••••••••••••"
						oninput={(value: string) => (accessToken = value)}
					/>
					<AsyncButton
						style="pop"
						disabled={accessToken.trim().length === 0}
						action={async () => {
							await userService.setUserAccessToken(accessToken.trim(), true);
							accessToken = "";
						}}
					>
						Authorize access token
					</AsyncButton>
				</div>

				<p class="text-12 text-body clr-text-2 m-t-16">
					An access token is required to use GitButler's smart automation features, including
					intelligent branch creation and commit message generation.
				</p>
			</div>
		</div>
	</CardGroup.Item>
{/if}

<style lang="postcss">
	.token-svg {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 110px;
		height: 100px;
		border-radius: var(--radius-m);
		background-color: var(--art-scene-bg);
	}

	.token-content {
		display: flex;
		gap: 24px;
	}

	.info-section {
		display: flex;
		flex: 1;
		flex-direction: column;
	}

	.token-fields {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
</style>
