<script>
	import HeaderLink from '$home/components/HeaderLink.svelte';
	import HeaderMobileLink from '$home/components/HeaderMobileLink.svelte';
	import * as jsonLinks from '$home/data/links.json';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { fly } from 'svelte/transition';
	import { env } from '$env/dynamic/public';

	let isMobileMenuOpen = $state(false);

	function handleBurgerClick() {
		isMobileMenuOpen = !isMobileMenuOpen;
	}

	const authService = inject(AUTH_SERVICE);
	let token = $derived(authService.tokenReadable);
</script>

<header class="header">
	<div class="logo">
		<!-- prettier-ignore -->
		<svg class="logo-sign" width="50" height="50" viewBox="0 0 50 50" fill="none" xmlns="http://www.w3.org/2000/svg">
      <rect width="50" height="50" rx="10.7156" fill="#97EAE5"/>

      <path class="logo-sign__left-arrow" d="M28.5992 26.5753L10.5898 35.4371C9.39996 36.0226 8 35.1711 8 33.8619V16.1384C8 14.8292 9.39996 13.9777 10.5898 14.5632L28.5992 23.425C29.9181 24.074 29.9181 25.9263 28.5992 26.5753Z" fill="black"/>
      <path class="logo-sign__right-arrow" d="M21.4008 26.5753L39.4102 35.4371C40.6 36.0226 42 35.1711 42 33.8619V16.1384C42 14.8292 40.6 13.9777 39.4102 14.5632L21.4008 23.425C20.0819 24.074 20.0819 25.9263 21.4008 26.5753Z" fill="url(#paint0_radial_154_30984)"/>
      
			<defs>
      <radialGradient id="paint0_radial_154_30984" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(25.2654 25.0002) rotate(-179.999) scale(16.7659 17.0326)">
      <stop stop-opacity="0"/>
      <stop offset="1"/>
      </radialGradient>
      </defs>
      </svg>

		<!-- prettier-ignore -->
		<svg class="logo-word" viewBox="0 0 178 39" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d="M15.8787 39C5.23118 39 0 30.5964 0 19.9179C0 8.68214 5.87929 0.371429 15.6472 0.371429C18.9341 0.371429 21.5265 1.25357 23.9801 3.34286C24.6745 3.99286 25.23 3.80714 25.3226 2.925L25.5541 0.789287H26.6651L27.2669 13.9286H26.2022C24.906 6.45357 21.7117 2.41429 16.4342 2.41429C9.4439 2.41429 5.64782 9.56429 5.64782 19.3143C5.64782 29.2036 9.3976 36.7714 16.5731 36.7714C20.0451 36.7714 22.4524 34.8214 23.6097 33.0107V23.8643C23.6097 21.0321 22.4987 20.1964 14.9528 20.1964V19.0821H31.3408V20.1964C28.8409 20.1964 28.4706 20.8929 28.4706 23.8643V32.7786C25.8318 36.5393 21.3876 39 15.8787 39Z" fill="black"/>
      <path d="M38.1774 8.26429C36.4182 8.26429 35.1683 7.05714 35.1683 5.43214C35.1683 3.76072 36.4182 2.55357 38.1774 2.55357C39.8903 2.55357 41.1402 3.76072 41.1402 5.43214C41.1402 7.05714 39.8903 8.26429 38.1774 8.26429ZM40.7698 13.4179V33.475C40.7698 36.4464 41.2328 37.1429 44.0567 37.1429V38.2571H33.0388V37.1429C35.8627 37.1429 36.3257 36.4464 36.3257 33.475V19.175C36.3257 16.0643 36.2794 15.4143 33.5943 16.5286L33.1777 15.4607L39.9828 12.7214L40.7698 13.4179Z" fill="black"/>
      <path d="M58.6981 27.8571H59.9943C59.9943 35.1 58.5592 39 54.0225 39C50.4116 39 48.5135 36.8179 48.5135 30.3179V15.5536H45.273V13.8357C48.6524 13.7429 50.7819 11.2357 51.2449 6.87143H52.9114V13.65H58.4667V15.5536H53.004V29.1107C53.004 33.2429 53.4207 35.9821 55.5965 35.9821C57.6797 35.9821 58.6981 33.3821 58.6981 27.8571Z" fill="black"/>
      <path d="M75.6289 38.2571H61.602V37.1429C65.9073 37.1429 66.5091 36.3071 66.5091 33.475V5.89643C66.5091 3.06429 65.9073 2.22857 61.602 2.22857V1.11429H76.0919C82.8507 1.11429 87.3412 4.08571 87.3412 10.3071C87.3412 15.4607 83.9618 18.4786 79.4713 19.0357V19.2679C85.258 19.6393 89.0541 23.2607 89.0541 28.275C89.0541 35.6107 83.0359 38.2571 75.6289 38.2571ZM71.6477 4.45714V18.1071C72.805 18.2929 74.0087 18.2929 75.1197 18.2929C78.9158 18.2929 82.2489 15.8786 82.2489 10.4929C82.2489 5.2 78.8695 3.01786 75.2123 3.01786C74.3327 3.01786 73.3605 3.06429 72.5735 3.25C71.9254 3.38929 71.6477 3.80714 71.6477 4.45714ZM71.6477 20.3821V34.9143C71.6477 36.0286 72.8513 36.3536 75.0271 36.3536C79.3787 36.3536 83.8692 34.7286 83.8692 28.3214C83.8692 21.9143 78.7306 20.1964 75.166 20.1964C73.9624 20.1964 72.805 20.1964 71.6477 20.3821Z" fill="black"/>
      <path d="M106.555 39L105.629 38.2107L105.537 34.2643L105.398 34.2179C103.824 36.725 101.602 39 98.315 39C95.1207 39 93.269 36.4 93.269 32.3143V19.2214C93.269 16.2036 93.1301 15.6464 90.3062 16.5286L89.9821 15.5071L97.1113 13.1857L97.7594 13.7893V31.85C97.7594 34.5893 98.5927 35.8893 100.352 35.8893C102.204 35.8893 104.055 34.3107 105.537 32.3143V19.2214C105.537 16.2036 105.444 15.6464 102.574 16.5286L102.25 15.5071L109.425 13.1857L110.027 13.7893V32.7321C110.027 35.8429 110.12 36.4464 112.99 35.3786L113.407 36.4464L106.555 39Z" fill="black"/>
      <path d="M127.325 27.8571H128.621C128.621 35.1 127.186 39 122.649 39C119.038 39 117.14 36.8179 117.14 30.3179V15.5536H113.9V13.8357C117.279 13.7429 119.409 11.2357 119.872 6.87143H121.538V13.65H127.093V15.5536H121.631V29.1107C121.631 33.2429 122.047 35.9821 124.223 35.9821C126.306 35.9821 127.325 33.3821 127.325 27.8571Z" fill="black"/>
      <path d="M137.6 0.65V33.475C137.6 36.4464 138.017 37.1429 140.841 37.1429V38.2571H129.823V37.1429C132.647 37.1429 133.11 36.4464 133.11 33.475V6.63929C133.11 3.66786 133.11 3.06429 130.332 4.45714L129.869 3.43571L136.906 0L137.6 0.65Z" fill="black"/>
      <path d="M158.373 28.3214H159.484C158.512 35.7964 155.364 39 151.012 39C145.781 39 142.494 34.0321 142.494 26.3714C142.494 19.0357 145.735 12.8143 151.244 12.8143C156.66 12.8143 159.206 17.9214 159.206 24.7H146.985V24.7929C146.985 31.1071 148.929 35.5643 152.771 35.5643C154.855 35.5643 157.262 33.9393 158.373 28.3214ZM151.244 14.7643C148.466 14.7643 147.309 18.1536 147.031 22.7964H154.762C154.762 18.3393 153.975 14.7643 151.244 14.7643Z" fill="black"/>
      <path d="M175.5 12.8143C177.12 12.8143 178 13.7429 178 15.275C178 17.0857 176.935 18.2464 175.732 18.2464C174.898 18.2464 174.528 17.9214 173.926 17.2714C173.324 16.6679 173.093 16.4821 172.537 16.4821C171.473 16.4821 170.223 18.0143 169.251 20.8464V33.1964C169.251 36.3536 169.945 37.0964 174.158 37.0964V38.2571H161.658V37.1429C164.343 37.1429 164.806 36.4464 164.806 33.475V19.175C164.806 16.0643 164.806 15.4143 162.029 16.575L161.566 15.5071L165.269 14.0214L168.417 12.7214L169.251 13.4179V17.875H169.389C170.686 15.4143 172.954 12.8143 175.5 12.8143Z" fill="black"/>
    </svg>
	</div>

	<nav class="navigation-desktop">
		<section class="navigation-section">
			<HeaderLink label="Download">
				{#snippet dropdown()}
					{#each Object.values(jsonLinks.downloads) as download}
						<a href={download.url} download>
							<span>{download.label}</span>
							<svg
								width="16"
								height="15"
								viewBox="0 0 16 15"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path
									d="M15 6.60504L8 13.2101M8 13.2101L1 6.60504M8 13.2101V0M1 14H15"
									stroke="black"
									stroke-width="1.5"
								/>
							</svg>
						</a>
					{/each}
				{/snippet}
			</HeaderLink>
			<HeaderLink label="Docs" href={jsonLinks.resources.documentation.url} hrefTarget="_blank" />
			<HeaderLink label="Community">
				{#snippet dropdown()}
					{#each Object.values(jsonLinks.social) as communityLink}
						<a href={communityLink.url} target="_blank">
							<span>{communityLink.label}</span>
							<svg
								width="14"
								height="14"
								viewBox="0 0 14 14"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path d="M1 1L13 1M13 1L13 13M13 1L1 13" stroke="black" stroke-width="1.5" />
							</svg>
						</a>
					{/each}
				{/snippet}
			</HeaderLink>
			<HeaderLink label="Blog" href={jsonLinks.resources.blog.url} />
			<HeaderLink label="Jobs" href={jsonLinks.resources.jobs.url} />
		</section>

		<section class="navigation-section">
			<HeaderLink
				label="Source"
				icon="github"
				hideTextOnTablet
				href={jsonLinks.resources.github.url}
				hrefTarget="_blank"
			/>
			<HeaderLink
				label="Discord"
				icon="discord"
				hideTextOnTablet
				href={jsonLinks.social.discord.url}
				hrefTarget="_blank"
			/>
		</section>
		<section class="navigation-section">
			{#if $token}
				<HeaderLink label="My Dashboard" href="/" icon="dashboard" />
			{:else}
				<HeaderLink
					label="Sign up / Log in"
					href={`${env.PUBLIC_APP_HOST}cloud/login?callback=${window.location.href}`}
					icon="login"
				/>
			{/if}
		</section>
	</nav>

	<section class="navigation-mobile">
		<button
			aria-label="Hamburger Menu Toggle"
			type="button"
			class="burger"
			onclick={handleBurgerClick}
		></button>

		{#if isMobileMenuOpen}
			<nav class="mobile-menu" transition:fly={{ duration: 100, y: 10 }}>
				<HeaderMobileLink label="Downloads">
					{#snippet dropdown()}
						{#each Object.values(jsonLinks.downloads) as download}
							<a href={download.url} target="_blank">{download.label}</a>
						{/each}
					{/snippet}
				</HeaderMobileLink>
				<HeaderMobileLink
					label="Docs"
					href={jsonLinks.resources.documentation.url}
					hrefTarget="_blank"
				/>
				<HeaderMobileLink label="Community">
					{#snippet dropdown()}
						{#each Object.values(jsonLinks.social) as communityLink}
							<a href={communityLink.url} target="_blank">{communityLink.label}</a>
						{/each}
					{/snippet}
				</HeaderMobileLink>
				<HeaderMobileLink label="Blog" href={jsonLinks.resources.blog.url} />
				<HeaderMobileLink
					label="Source"
					icon="github"
					href={jsonLinks.resources.github.url}
					hrefTarget="_blank"
				/>
				<HeaderMobileLink
					label="Discord"
					icon="discord"
					href={jsonLinks.social.discord.url}
					hrefTarget="_blank"
				/>
				<HeaderMobileLink label="Jobs" href={jsonLinks.resources.jobs.url} />
				<section class="navigation-section">
					{#if $token}
						<HeaderMobileLink label="My Dashboard" href="/" icon="dashboard" />
					{:else}
						<HeaderMobileLink
							label="Sign up / Log in"
							href={`${env.PUBLIC_APP_HOST}cloud/login?callback=${window.location.href}`}
							icon="login"
						/>
					{/if}
				</section>
			</nav>
		{/if}
	</section>
</header>

<style lang="scss">
	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-top: 50px;

		@media (max-width: 1200px) {
			padding-top: 30px;
		}

		@media (max-width: 800px) {
			padding-top: 20px;
		}
	}

	.logo {
		display: flex;
		z-index: 11;
		position: relative;
		align-items: center;
		gap: 20px;

		@media (max-width: 600px) {
			gap: 16px;
		}
	}

	.logo-word {
		width: 178px;
		height: 39px;

		@media (max-width: 600px) {
			width: 150px;
		}
	}

	.logo-sign {
		&:hover {
			animation: sign-animation 0.4s ease-in-out forwards;

			.logo-sign__left-arrow {
				animation: left-arrow-animation 0.4s ease-in-out forwards;
			}

			.logo-sign__right-arrow {
				animation: right-arrow-animation 0.4s ease-in-out forwards;
			}
		}
	}

	@keyframes sign-animation {
		0% {
			transform: scale(1);
		}
		50% {
			transform: scale(0.96);
		}
		100% {
			transform: scale(1);
		}
	}

	@keyframes left-arrow-animation {
		0% {
			transform: translateX(0);
		}
		50% {
			transform: translateX(2px);
		}
		100% {
			transform: translateX(0);
		}
	}

	@keyframes right-arrow-animation {
		0% {
			transform: translateX(0);
		}
		50% {
			transform: translateX(-2px);
		}
		100% {
			transform: translateX(0);
		}
	}

	// NAVIGATION

	.navigation-desktop {
		display: flex;
		align-items: center;
		gap: 32px;

		@media (max-width: 1280px) {
			display: none;
		}
	}

	.navigation-section {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.navigation-mobile {
		display: none;

		@media (max-width: 1280px) {
			display: flex;
			align-items: center;
			gap: 60px;
		}
	}

	// BURGER

	.burger {
		display: flex;
		z-index: 11;
		position: relative;
		align-items: center;
		justify-content: center;
		width: 40px;
		height: 40px;
		border: none;
		background: transparent;
		cursor: pointer;

		&::before,
		&::after {
			position: absolute;
			width: 80%;
			height: 2px;
			background-color: var(--clr-black);
			content: '';
			transition: transform 0.1s ease-in-out;
		}

		&::before {
			transform: translateY(-6px);
		}

		&::after {
			transform: translateY(6px);
		}

		&:hover,
		&:focus-within {
			&::before {
				transform: translateY(-4px);
			}

			&::after {
				transform: translateY(4px);
			}
		}
	}

	.mobile-menu {
		display: flex;
		z-index: 10;
		position: absolute;
		top: 0;
		left: 0;
		flex-direction: column;
		width: 100%;
		padding: 120px 40px 40px;
		gap: 2px;
		background-color: var(--clr-white);

		@media (max-width: 800px) {
			padding: 100px 20px 20px;
		}
	}
</style>
