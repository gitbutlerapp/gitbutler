<script lang="ts">
	import { Button } from "@gitbutler/ui";
	import { env } from "$env/dynamic/public";
</script>

<div class="supporter-card">
	<h3 class="text-15 text-bold">🎉 Danke, dass du uns unterstützt!</h3>
	<p class="text-12 text-body clr-text-2">
		Deine Unterstützung hilft uns, GitButler noch besser zu machen.<br />Wir schätzen deinen Beitrag.
	</p>
	<Button
		style="pop"
		onclick={() => (window.location.href = `${env.PUBLIC_APP_HOST}supporter/portal`)}
	>
		Abonnement verwalten
	</Button>
</div>

<style lang="postcss">
	.supporter-card {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding: 16px;
		gap: 12px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-1);
	}
</style>
