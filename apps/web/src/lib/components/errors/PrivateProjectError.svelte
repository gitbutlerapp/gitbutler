<script lang="ts">
	import { goto } from '$app/navigation';
	import { inject } from '@gitbutler/core/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, Icon } from '@gitbutler/ui';

	const routes = inject(WEB_ROUTES_SERVICE);

	function goToHome() {
		goto(routes.homePath());
	}
</script>

<div class="private-project-error">
	<div class="wrapper">
		<div class="icon-container">
			<Icon name="locked-small" size={48} />
		</div>
		<h2 class="title text-15 text-body text-bold">Private Project</h2>
		<p class="error-message text-13 text-body">
			You don't have permission to view this project. The project may be private or you may need to
			request access from the owner.
		</p>
		<div class="actions">
			<Button kind="outline" icon="home" onclick={goToHome}>Go to home</Button>
		</div>
	</div>
</div>

<style lang="postcss">
	.private-project-error {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		padding: 32px;
	}

	.wrapper {
		display: flex;
		flex-direction: column;
		align-items: center;
		max-width: 400px;
		padding: 32px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		text-align: center;
	}

	.icon-container {
		margin-bottom: 16px;
		color: var(--clr-theme-err-element);
	}

	.title {
		margin-bottom: 12px;
	}

	.error-message {
		margin-bottom: 24px;
		color: var(--clr-text-2);
		line-height: 1.5;
	}

	.actions {
		display: flex;
	}
</style>
