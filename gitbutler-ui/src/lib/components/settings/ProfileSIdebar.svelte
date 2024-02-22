<script lang="ts">
	// import Button from '$lib/components/Button.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import Login from '$lib/components/Login.svelte';
	import type { UserService } from '$lib/stores/user';
	import { goto } from '$app/navigation';

	export let userService: UserService;
	export let currentSection: 'profile' | 'git-stuff' | 'telemetry' | 'integrations' = 'profile';
	export let showIntegrations = false;

	const onMenuClick = (section: 'profile' | 'git-stuff' | 'telemetry' | 'integrations') => {
		currentSection = section;
	};
</script>

<aside class="profile-sidebar" data-tauri-drag-region>
	<section class="profile-sidebar__top">
		<button
			class="back-btn"
			on:click={() => {
				if (history.length > 0) {
					history.back();
				} else {
					goto('/');
				}
			}}
		>
			<div class="back-btn__icon">
				<Icon name="chevron-left" />
			</div>
			<span class="text-base-14 text-semibold">Back</span>
		</button>

		<div class="profile-sidebar__menu-wrapper">
			<h2 class="profile-sidebar__title text-base-18 text-bold">Preferences</h2>
			<ul class="profile-sidebar__menu">
				<li>
					<button
						class="profile-sidebar__menu-item"
						class:item_selected={currentSection === 'profile'}
						on:click={() => onMenuClick('profile')}
					>
						<Icon name="profile" />
						<span class="text-base-14 text-semibold">Profile</span>
					</button>
				</li>
				<li>
					<button
						class="profile-sidebar__menu-item"
						class:item_selected={currentSection === 'git-stuff'}
						on:click={() => onMenuClick('git-stuff')}
					>
						<Icon name="git" />
						<span class="text-base-14 text-semibold">Git Stuff</span>
					</button>
				</li>
				<li>
					<button
						class="profile-sidebar__menu-item"
						class:item_selected={currentSection === 'telemetry'}
						on:click={() => onMenuClick('telemetry')}
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
							on:click={() => onMenuClick('integrations')}
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
				href="https://discord.gg/MmFkmaJ42D"
				target="_blank"
				rel="noreferrer"
			>
				<span class="text-base-14 text-bold">Join our Discord</span>
				<Icon name="discord" />
			</a>
			<a
				class="social-banner"
				href="mailto:hello@gitbutler.com?subject=Feedback or question!"
				target="_blank"
			>
				<span class="text-base-14 text-bold">Contact us</span>
				<Icon name="mail" />
			</a>
		</div>
		<Login
			{userService}
			wide
			on:logout={() => {
				currentSection = 'profile';
			}}
		/>
	</section>
</aside>

<style lang="post-css">
	.profile-sidebar {
		user-select: none;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		padding: var(--space-48) var(--space-20) var(--space-20) var(--space-20);
		border-right: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-pale);
		height: 100%;
		width: 280px;
	}

	.back-btn {
		display: flex;
		align-items: center;
		gap: var(--space-6);
		width: fit-content;
		margin-left: calc(var(--space-6) * -1);
		transition: transform var(--transition-fast);

		&:hover {
			& span {
				opacity: 0.8;
			}

			& .back-btn__icon {
				transform: translateX(calc(var(--space-2) * -1));
			}
		}

		& span {
			opacity: 0.4;
			transition: opacity var(--transition-fast);
		}
	}

	.back-btn__icon {
		transition: transform var(--transition-fast);
	}

	/* TOP */

	.profile-sidebar__top {
		display: flex;
		flex-direction: column;
		gap: var(--space-20);
	}

	.profile-sidebar__title {
		color: var(--clr-theme-scale-ntrl-0);
	}

	/* MENU */

	.profile-sidebar__menu-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--space-16);
	}

	.profile-sidebar__menu {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.profile-sidebar__menu-item {
		display: flex;
		align-items: center;
		gap: var(--space-10);
		padding: var(--space-8);
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
				var(--clr-theme-container-pale),
				var(--darken-tint-light)
			);
		}

		& span {
			color: var(--clr-theme-scale-ntrl-0);
		}
	}

	.item_selected {
		background-color: color-mix(in srgb, var(--clr-theme-container-pale), var(--darken-tint-light));
		color: var(--clr-theme-scale-ntrl-0);
	}

	/* BOTTOM */
	.profile-sidebar__bottom {
		display: flex;
		flex-direction: column;
		gap: var(--space-24);
	}

	/* BANNERS */

	.social-banners {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}

	.social-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-16);
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-pale);
		color: var(--clr-theme-scale-ntrl-30);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-pale),
				var(--darken-tint-light)
			);
		}
	}
</style>
