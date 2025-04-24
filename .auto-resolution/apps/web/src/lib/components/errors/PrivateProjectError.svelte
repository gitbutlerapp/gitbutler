<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { goto } from '$app/navigation';

	const routes = getContext(WebRoutesService);

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
		height: 100%;
		width: 100%;
		padding: 32px;
	}

	.wrapper {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		max-width: 400px;
		padding: 32px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.icon-container {
		color: var(--clr-scale-red-10);
		margin-bottom: 16px;
	}

	.title {
		margin-bottom: 12px;
	}

	.error-message {
		color: var(--clr-text-2);
		margin-bottom: 24px;
		line-height: 1.5;
	}

	.actions {
		display: flex;
	}
</style>
