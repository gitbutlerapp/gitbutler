<script lang="ts">
	import loginSvg from "$lib/assets/login-modal.svg?raw";
	import { goToLogin, goToSignup } from "$lib/auth/pageBeforeLogin";
	import { inject } from "@gitbutler/core/context";
	import { WEB_ROUTES_SERVICE } from "@gitbutler/shared/routing/webRoutes.svelte";
	import { Button, Modal } from "@gitbutler/ui-svelte";

	import { type Snippet } from "svelte";

	const routes = inject(WEB_ROUTES_SERVICE);

	let modal: ReturnType<typeof Modal> | undefined = $state();

	interface Props {
		children?: Snippet;
	}

	const { children }: Props = $props();

	export function show() {
		modal?.show();
	}
</script>

<Modal bind:this={modal} noPadding closeButton width={550}>
	<div class="login-modal">
		<div class="login-modal__illustration">
			{@html loginSvg}
		</div>
		<div class="login-modal__content">
			<h2 class="text-18 text-bold login-modal__title">🔒 Log in to continue</h2>
			<p class="text-13 text-body login-modal__text">
				{#if children}
					{@render children()}
				{:else}
					You need to be logged in to unlock full access to all features.
				{/if}
			</p>

			<div class="login-modal__actions">
				<Button style="pop" onclick={() => goToLogin(routes)}>Log in</Button>
				<Button kind="outline" onclick={() => goToSignup(routes)}>Sign up</Button>
			</div>
		</div>
	</div>
</Modal>

<style lang="postcss">
	.login-modal {
		display: flex;
	}

	.login-modal__illustration {
		width: 240px;
		height: 300px;
		background-color: var(--art-scene-bg);

		@media (--tablet-viewport) {
			display: none;
		}
	}

	.login-modal__content {
		display: flex;
		flex: 1;
		flex-direction: column;
		justify-content: center;
		padding: 30px;
	}

	.login-modal__title {
		margin-bottom: 16px;
	}

	.login-modal__text {
		margin-bottom: 30px;
	}

	.login-modal__actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
