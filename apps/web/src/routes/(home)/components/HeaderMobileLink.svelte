<script lang="ts">
	import { slide } from 'svelte/transition';

	interface Props {
		label?: string;
		icon?: 'github' | 'discord' | 'dashboard' | 'login' | undefined;
		hideTextOnTablet?: boolean;
		href?: string;
		hrefTarget?: string;
		dropdown?: import('svelte').Snippet;
	}

	let {
		label = 'Label',
		icon = undefined,
		hideTextOnTablet = false,
		href = '#',
		hrefTarget = '_self',
		dropdown
	}: Props = $props();

	const icons = {
		github:
			'M10.0074 1.5C5.02656 1.5 1 5.39582 1 10.2155C1 14.0681 3.57996 17.3292 7.15904 18.4835C7.60652 18.5702 7.77043 18.2959 7.77043 18.0652C7.77043 17.8631 7.75568 17.1706 7.75568 16.449C5.25002 16.9685 4.72824 15.41 4.72824 15.41C4.32557 14.3999 3.72893 14.1403 3.72893 14.1403C2.90883 13.6064 3.78867 13.6064 3.78867 13.6064C4.69837 13.6642 5.17572 14.501 5.17572 14.501C5.98089 15.8285 7.27833 15.4534 7.8003 15.2225C7.87478 14.6597 8.11355 14.2701 8.36706 14.0537C6.36863 13.8517 4.26602 13.1014 4.26602 9.75364C4.26602 8.80129 4.6237 8.02213 5.19047 7.41615C5.10105 7.19976 4.7878 6.30496 5.28008 5.10735C5.28008 5.10735 6.04062 4.87643 7.75549 6.00197C9.22525 5.62006 10.7896 5.61702 12.2592 6.00197C13.9743 4.87643 14.7348 5.10735 14.7348 5.10735C15.2271 6.30496 14.9137 7.19976 14.8242 7.41615C15.4059 8.02213 15.7489 8.80129 15.7489 9.75364C15.7489 13.1014 13.6463 13.8372 11.6329 14.0537C11.9611 14.3279 12.2443 14.8472 12.2443 15.6698C12.2443 16.8385 12.2295 17.7765 12.2295 18.065C12.2295 18.2959 12.3936 18.5702 12.8409 18.4836C16.42 17.3291 19 14.0681 19 10.2155C19.0147 5.39582 14.9734 1.5 10.0074 1.5Z',
		discord:
			'M16.9419 3.75623C15.6279 3.16091 14.2407 2.73857 12.8158 2.5C12.6208 2.84494 12.4443 3.19983 12.2872 3.5632C10.7694 3.33686 9.22584 3.33686 7.70801 3.5632C7.55079 3.19987 7.37437 2.84498 7.17946 2.5C5.75361 2.74059 4.3655 3.16393 3.05016 3.75934C0.43887 7.5825 -0.269009 11.3107 0.0849305 14.986C1.61417 16.1041 3.32582 16.9544 5.14548 17.5C5.55522 16.9547 5.91778 16.3761 6.22933 15.7705C5.63759 15.5518 5.06646 15.282 4.52255 14.9642C4.6657 14.8615 4.8057 14.7556 4.94099 14.6529C6.52364 15.3894 8.25103 15.7713 9.99997 15.7713C11.7489 15.7713 13.4763 15.3894 15.0589 14.6529C15.1958 14.7634 15.3358 14.8692 15.4774 14.9642C14.9324 15.2825 14.3602 15.5529 13.7675 15.7721C14.0786 16.3774 14.4412 16.9555 14.8513 17.5C16.6725 16.9566 18.3855 16.1067 19.915 14.9875C20.3303 10.7254 19.2055 7.03144 16.9419 3.75623ZM6.67765 12.7257C5.69134 12.7257 4.87649 11.84 4.87649 10.7503C4.87649 9.66065 5.66302 8.76712 6.6745 8.76712C7.68599 8.76712 8.49454 9.66065 8.47724 10.7503C8.45993 11.84 7.68284 12.7257 6.67765 12.7257ZM13.3223 12.7257C12.3344 12.7257 11.5227 11.84 11.5227 10.7503C11.5227 9.66065 12.3092 8.76712 13.3223 8.76712C14.3353 8.76712 15.1376 9.66065 15.1203 10.7503C15.103 11.84 14.3275 12.7257 13.3223 12.7257Z',
		dashboard:
			'M5 3C3.89543 3 3 3.89543 3 5V5.66667C3 6.77124 3.89543 7.66667 5 7.66667L6.83333 7.66667C7.9379 7.66667 8.83333 6.77124 8.83333 5.66667V5C8.83333 3.89543 7.9379 3 6.83333 3H5ZM5 10C3.89543 10 3 10.8954 3 12L3 15C3 16.1046 3.89543 17 5 17H6.83333C7.9379 17 8.83333 16.1046 8.83333 15L8.83333 12C8.83333 10.8954 7.9379 10 6.83333 10H5ZM11.1667 5C11.1667 3.89543 12.0621 3 13.1667 3H15C16.1046 3 17 3.89543 17 5V8C17 9.10457 16.1046 10 15 10L13.1667 10C12.0621 10 11.1667 9.10457 11.1667 8V5ZM13.1667 12.333C12.0621 12.333 11.1667 13.2284 11.1667 14.333V14.9997C11.1667 16.1042 12.0621 16.9997 13.1667 16.9997H15C16.1046 16.9997 17 16.1042 17 14.9997V14.333C17 13.2284 16.1046 12.333 15 12.333H13.1667Z',
		login:
			'M4.98113 6.44444C4.98113 3.98985 6.88181 2 9.22641 2H13.7547C16.0993 2 18 3.98985 18 6.44444V13.5556C18 16.0102 16.0993 18 13.7547 18H9.22641C6.88181 18 4.98113 16.0102 4.98113 13.5556H6.67925C6.67925 15.0283 7.81965 16.2222 9.22641 16.2222H13.7547C15.1615 16.2222 16.3019 15.0283 16.3019 13.5556V6.44444C16.3019 4.97169 15.1615 3.77778 13.7547 3.77778H9.22641C7.81965 3.77778 6.67925 4.97169 6.67925 6.44444H4.98113Z  M10.3928 5.8159L14.3894 10L10.3928 14.1841L9.19208 12.927L11.1389 10.8889H3V9.11111H11.1389L9.19208 7.07298L10.3928 5.8159Z'
	};

	let isOpen = $state(false);

	function handleClick() {
		isOpen = !isOpen;
	}
</script>

{#if dropdown}
	<div class="link" role="button" tabindex="0" onclick={handleClick} onkeydown={handleClick}>
		<span>{label}</span>
		<svg
			class="dropdown-icon"
			width="12"
			height="16"
			viewBox="0 0 12 16"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path
				class="dropdown-icon__arrow-up"
				d="M10.5156 10L6.01562 14L1.51562 10"
				stroke="black"
				stroke-width="1.5"
			/>
			<path
				class="dropdown-icon__arrow-down"
				d="M10.5156 6L6.01563 2L1.51562 6"
				stroke="black"
				stroke-width="1.5"
			/>
		</svg>
	</div>
	{#if isOpen}
		<div class="dropdown-container" transition:slide={{ duration: 150 }}>
			{@render dropdown?.()}
		</div>
	{/if}
{:else}
	<a class="link" {href} target={hrefTarget}>
		<span class:hide-on-tablet={hideTextOnTablet}> {label} </span>

		{#if icon}
			<svg class="icon" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
				<path fill-rule="evenodd" clip-rule="evenodd" d={icons[icon]} />
			</svg>
		{/if}
	</a>
{/if}

<style lang="scss">
	.link {
		display: flex;
		position: relative;
		align-items: center;
		padding: 10px 0;
		gap: 8px;
		background-color: transparent;
		color: var(--clr-black);
		font-weight: 500;
		font-size: 20px;
		text-decoration: none;
		text-transform: uppercase;
		cursor: pointer;
		user-select: none;
	}

	.dropdown-container {
		// display: none;
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		margin-bottom: 10px;
		padding: 12px;
		border-radius: 12px;
		background-color: var(--clr-light-gray);
		// border: 1px solid var(--clr-gray);

		:global(a) {
			display: block;
			padding: 8px;
			border-radius: 6px;
			color: var(--clr-black);
			font-weight: 500;
			font-size: 18px;
			text-decoration: none;
			text-transform: uppercase;
			transition: background-color 0.05s ease-in-out;
		}
	}

	.dropdown-icon {
		fill: none;
	}

	.icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
	}

	.hide-on-tablet {
		@media (max-width: 1300px) {
			display: none;
		}
	}
</style>
