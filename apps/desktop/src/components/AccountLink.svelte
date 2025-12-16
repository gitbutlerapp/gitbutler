<script lang="ts">
	import { useSettingsModal } from '$lib/settings/settingsModal.svelte';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/core/context';
	import { Icon } from '@gitbutler/ui';

	const user = inject(USER);
	const { openGeneralSettings } = useSettingsModal();
</script>

<button type="button" class="profile-btn" onclick={async () => openGeneralSettings()}>
	{#if $user?.picture}
		<img class="profile-picture" src={$user.picture} alt="Avatar" referrerpolicy="no-referrer" />
	{:else}
		<div class="anon-icon">
			<Icon name="profile" />
		</div>
	{/if}
</button>

<style lang="postcss">
	.profile-btn {
		display: flex;
		align-items: center;

		width: 28px;
		height: 28px;
		overflow-x: hidden;
		overflow: hidden;
		gap: 8px;
		border-radius: var(--radius-m);
		background: var(--clr-theme-pop-element);
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast),
			filter var(--transition-fast);

		&:hover {
			background: var(--hover-pop);
		}
	}
	.anon-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2px;
		color: var(--clr-theme-pop-on-element);
	}

	/* MODIFIERS */
	.anon-icon {
		width: 100%;
		height: 100%;
	}
</style>
