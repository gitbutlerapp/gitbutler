<script lang="ts">
	import SupportersBanner from './SupportersBanner.svelte';
	import IconButton from '../IconButton.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import { goto } from '$app/navigation';

	export let currentSection: 'profile' | 'git-stuff' | 'telemetry' | 'integrations' = 'profile';
	export let showIntegrations = false;

	function onMenuClick(section: 'profile' | 'git-stuff' | 'telemetry' | 'integrations') {
		currentSection = section;
	}
</script>

<aside class="profile-sidebar" data-tauri-drag-region>
	<section class="profile-sidebar__top">
		<div class="profile-sidebar__menu-wrapper">
			<div class="profile-sidebar__header">
				<div class="back-btn__icon">
					<IconButton
						icon="chevron-left"
						size="m"
						on:mousedown={() => {
							if (history.length > 0) {
								history.back();
							} else {
								goto('/');
							}
						}}
					/>
				</div>
				<h2 class="profile-sidebar__title text-base-18 text-bold">Preferences</h2>
			</div>

			<ul class="profile-sidebar__menu">
				<li>
					<button
						class="profile-sidebar__menu-item"
						class:item_selected={currentSection === 'profile'}
						on:mousedown={() => onMenuClick('profile')}
					>
						<Icon name="profile" />
						<span class="text-base-14 text-semibold">Profile</span>
					</button>
				</li>
				<li>
					<button
						class="profile-sidebar__menu-item"
						class:item_selected={currentSection === 'git-stuff'}
						on:mousedown={() => onMenuClick('git-stuff')}
					>
						<Icon name="git" />
						<span class="text-base-14 text-semibold">Git Stuff</span>
					</button>
				</li>
				<li>
					<button
						class="profile-sidebar__menu-item"
						class:item_selected={currentSection === 'telemetry'}
						on:mousedown={() => onMenuClick('telemetry')}
					>
						<Icon name="stat" />
						<span class="text-base-14 text-semibold">Telemetry</span>
					</button>
				</li>
				{#if showIntegrations}
					<li>
						<button
							class="profile-sidebar__menu-item"
							class:item_selected={currentSection === 'integrations'}
							on:mousedown={() => onMenuClick('integrations')}
						>
							<Icon name="integrations" />
							<span class="text-base-14 text-semibold">Integrations</span>
						</button>
					</li>
				{/if}
			</ul>
		</div>
	</section>

	<section class="profile-sidebar__bottom">
		<div class="social-banners">
			<a
				class="social-banner"
				href="mailto:hello@gitbutler.com?subject=Feedback or question!"
				target="_blank"
			>
				<span class="text-base-14 text-bold">Contact us</span>
				<Icon name="mail" />
			</a>
			<a
				class="social-banner"
				href="https://discord.gg/MmFkmaJ42D"
				target="_blank"
				rel="noreferrer"
			>
				<span class="text-base-14 text-bold">Join our Discord</span>
				<Icon name="discord" />
			</a>
		</div>

		<SupportersBanner />
	</section>
</aside>

<style lang="post-css">
	.profile-sidebar {
		user-select: none;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		padding: calc(var(--size-40) + var(--size-4)) var(--size-14) var(--size-14) var(--size-14);
		border-right: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-light);
		height: 100%;
		width: 16rem;
	}

	.profile-sidebar__header {
		display: flex;
		align-items: center;
		gap: var(--size-8);
	}

	/* TOP */

	.profile-sidebar__top {
		display: flex;
		flex-direction: column;
		gap: var(--size-20);
	}

	.profile-sidebar__title {
		color: var(--clr-theme-scale-ntrl-0);
	}

	/* MENU */

	.profile-sidebar__menu-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--size-20);
	}

	.profile-sidebar__menu {
		display: flex;
		flex-direction: column;
		gap: var(--size-2);
	}

	.profile-sidebar__menu-item {
		display: flex;
		align-items: center;
		gap: var(--size-10);
		padding: var(--size-8);
		border-radius: var(--radius-m);
		width: 100%;
		color: var(--clr-theme-scale-ntrl-30);
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast);

		&:not(.item_selected):hover {
			transition: none;
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-light),
				var(--darken-tint-light)
			);
		}

		& span {
			color: var(--clr-theme-scale-ntrl-0);
		}
	}

	.item_selected {
		background-color: color-mix(
			in srgb,
			var(--clr-theme-container-light),
			var(--darken-tint-light)
		);
		color: var(--clr-theme-scale-ntrl-0);
	}

	/* BOTTOM */
	.profile-sidebar__bottom {
		display: flex;
		flex-direction: column;
		gap: var(--size-24);
	}

	/* BANNERS */

	.social-banners {
		display: flex;
		flex-direction: column;
		gap: var(--size-6);
	}

	.social-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--size-16);
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-light);
		color: var(--clr-theme-scale-ntrl-30);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-light),
				var(--darken-tint-extralight)
			);
		}
	}
</style>
