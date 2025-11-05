<script lang="ts">
	import { useSettingsModal } from '$lib/settings/settingsModal.svelte';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/core/context';
	import { Icon } from '@gitbutler/ui';

	interface Props {
		pop?: boolean;
		isNavCollapsed?: boolean;
	}

	const { pop = false, isNavCollapsed = false }: Props = $props();

	const user = inject(USER);
	const { openGeneralSettings } = useSettingsModal();
</script>

<button
	type="button"
	class="profile-btn"
	class:pop
	onclick={async () => openGeneralSettings()}
	class:collapsed={isNavCollapsed}
>
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
		color: var(--clr-scale-ntrl-50);
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast),
			filter var(--transition-fast);

		&.pop {
			background: var(--clr-scale-pop-70);
			color: var(--clr-scale-pop-10);

			&:hover {
				background: color-mix(in srgb, var(--clr-scale-pop-70) 90%, var(--clr-scale-pop-50));
				color: var(--clr-scale-pop-10);
			}
		}

		&:hover {
			background-color: var(--clr-bg-1-muted);
			color: var(--clr-scale-ntrl-40);
		}
	}
	.anon-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2px;
		background: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
	}

	/* MODIFIERS */
	.profile-btn.collapsed {
		height: auto;
		padding: 8px;
		overflow-x: initial;
	}

	.profile-btn.collapsed .anon-icon,
	.profile-btn.collapsed .profile-picture {
		width: 24px;
		height: 24px;
	}
</style>
