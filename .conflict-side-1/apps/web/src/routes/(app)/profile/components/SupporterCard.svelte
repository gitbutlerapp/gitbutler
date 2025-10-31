<script lang="ts">
	import { Button } from '@gitbutler/ui';
	import { env } from '$env/dynamic/public';
</script>

<div class="supporter-card">
	<h3 class="text-15 text-bold">🎉 Thank you for being a supporter!</h3>
	<p class="text-12 text-body clr-text-2">
		Your support helps us build a better GitButler.<br />We appreciate your contribution.
	</p>
	<Button
		style="pop"
		onclick={() => (window.location.href = `${env.PUBLIC_APP_HOST}supporter/portal`)}
	>
		Manage your subscription
	</Button>
</div>

<style lang="postcss">
	.supporter-card {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding: 16px;
		gap: 12px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-scale-pop-95);
	}
</style>
