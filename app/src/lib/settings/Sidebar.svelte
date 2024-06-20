<script lang="ts">
	import SupportersBanner from './SupportersBanner.svelte';
	import Button from '$lib/shared/Button.svelte';
	import Icon from '$lib/shared/Icon.svelte';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '$lib/utils/context';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	const userService = getContext(UserService);
	const user = userService.user;

	let currentSection: string | undefined;
	$: currentSection = getPageName($page.url.pathname);

	const settingsPageRegExp = /\/settings\/(.*?)(?:$|\/)/;

	function getPageName(pathname: string) {
		const matches = pathname.match(settingsPageRegExp);

		return matches?.[1];
	}

	function onMenuClick(section: string) {
		goto(`/settings/${section}`, { replaceState: true });
	}
</script>

<aside class="profile-sidebar" data-tauri-drag-region>
	<section class="profile-sidebar__top">
		<div class="profile-sidebar__menu-wrapper">
			<div class="profile-sidebar__header">
				<div class="back-btn__icon">
					<Button
						icon="chevron-left"
						style="ghost"
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
						class:item_selected={currentSection === 'git'}
						on:mousedown={() => onMenuClick('git')}
					>
						<Icon name="git" />
						<span class="text-base-14 text-semibold">Git stuff</span>
					</button>
				</li>

				{#if $user}
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
				<li>
					<button
						class="profile-sidebar__menu-item"
						class:item_selected={currentSection === 'ai'}
						on:mousedown={() => onMenuClick('ai')}
					>
						<Icon name="ai" />
						<span class="text-base-14 text-semibold">AI options</span>
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
				<li>
					<button
						class="profile-sidebar__menu-item"
						class:item_selected={currentSection === 'experimental'}
						on:mousedown={() => onMenuClick('experimental')}
					>
						<Icon name="idea" />
						<span class="text-base-14 text-semibold">Experimental</span>
					</button>
				</li>
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

<style lang="postcss">
	.profile-sidebar {
		user-select: none;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		padding: 40px 14px 14px 14px;
		border-right: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		height: 100%;
		width: 256px;
	}

	.profile-sidebar__header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	/* TOP */

	.profile-sidebar__top {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.profile-sidebar__title {
		color: var(--clr-scale-ntrl-0);
	}

	/* MENU */

	.profile-sidebar__menu-wrapper {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.profile-sidebar__menu {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.profile-sidebar__menu-item {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 8px;
		border-radius: var(--radius-m);
		width: 100%;
		color: var(--clr-scale-ntrl-30);
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast);

		&:not(.item_selected):hover {
			transition: none;
			background-color: var(--clr-bg-1-muted);
		}

		& span {
			color: var(--clr-scale-ntrl-0);
		}
	}

	.item_selected {
		background-color: var(--clr-bg-2);
		color: var(--clr-scale-ntrl-0);
	}

	/* BOTTOM */
	.profile-sidebar__bottom {
		display: flex;
		flex-direction: column;
		gap: 24px;
	}

	/* BANNERS */
	.social-banners {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.social-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		color: var(--clr-scale-ntrl-30);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}
</style>
