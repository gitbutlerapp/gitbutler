<script lang="ts">
	import StackTabs from '$components/StackTabs.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const settingsService = getContext(SettingsService);
	const settingsStore = settingsService.appSettings;

	const { data }: { data: PageData } = $props();
	const projectId = $derived(data.projectId);
	const stackId = $derived(page.params.stackId);

	// Redirect to board if we have switched away from V3 feature.
	$effect(() => {
		if ($settingsStore && !$settingsStore.featureFlags.v3) {
			goto(`/${data.projectId}/board`);
		}
	});
</script>

<div class="workspace">
	<div class="left">
		<div class="left-header">
			<div class="text-14 text-semibold">Uncommitted changes</div>
			<Button kind="ghost" icon="sidebar-unfold" />
		</div>

		<div class="left-body">
			<svg
				width="120"
				height="100"
				viewBox="0 0 120 100"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<g clip-path="url(#clip0_3811_12702)">
					<path
						opacity="0.25"
						d="M34.9721 79.0019L80.0676 76.1588C86.5068 75.7528 89.7264 75.5498 92.0114 73.9701C93.3532 73.0424 94.4477 71.8007 95.1994 70.353C96.4797 67.8876 96.2767 64.668 95.8707 58.2289L94.2907 33.1676C93.8847 26.7285 93.6817 23.5089 92.102 21.2238C91.1743 19.882 89.9325 18.7876 88.4849 18.0358C86.0195 16.7556 82.7999 16.9586 76.3608 17.3645L49.3649 19.0665C47.8159 19.1642 47.0415 19.213 46.3575 19.3534C42.9543 20.0517 40.1574 22.4662 38.9699 25.7311C38.7312 26.3872 38.5699 27.1463 38.2472 28.6644L35.5957 41.1388C35.1265 43.3462 34.892 44.4498 34.4482 45.4324C33.7959 46.8766 32.8098 48.1451 31.571 49.1333C30.7282 49.8057 29.7025 50.3121 27.6511 51.3248C24.9728 52.6471 23.6336 53.3082 22.6206 54.212C21.0266 55.6341 19.9298 57.5292 19.491 59.6198C19.2121 60.9484 19.3048 62.4177 19.4901 65.3563C19.7592 69.6252 19.8938 71.7597 20.6434 73.4219C21.8252 76.0423 24.0775 78.0274 26.8255 78.8707C28.5687 79.4056 30.7032 79.2711 34.9721 79.0019Z"
						fill="#D4D0CE"
					/>
					<path
						d="M50.3304 47.3764L61.1532 55.4794C62.7942 56.708 65.0983 56.4972 66.4891 54.9913L82.9404 37.1779"
						stroke="#CDC9C6"
						stroke-width="2"
					/>
					<path
						opacity="0.21"
						d="M36.1051 91.1107L47.659 78.2018L66.8838 76.9898L82.7672 88.1687L36.1051 91.1107Z"
						fill="#867E79"
					/>
					<path
						opacity="0.3"
						d="M1.9933 22.2747L6.56951 18.386C9.20709 16.1447 10.6359 12.7926 10.4264 9.33772L10.0437 3.02781L14.0383 7.92725C16.2255 10.6099 19.5478 12.1066 23.0062 11.9674L29.0067 11.7258L24.4304 15.6145C21.7929 17.8558 20.364 21.2078 20.5736 24.6628L20.9563 30.9727L16.9617 26.0732C14.7745 23.3906 11.4522 21.8939 7.99373 22.0331L1.9933 22.2747Z"
						fill="#3CB4AE"
					/>
					<path
						opacity="0.3"
						d="M114.049 76.3141L119.746 88.5636L108.951 96.6858L103.254 84.4363L114.049 76.3141Z"
						fill="#3CB4AE"
					/>
				</g>
				<defs>
					<clipPath id="clip0_3811_12702">
						<rect width="120" height="100" fill="white" />
					</clipPath>
				</defs>
			</svg>
			<div class="text-12 text-body helper-text">
				<div>You're all caught up!</div>
				<div>No files need committing</div>
			</div>
		</div>
	</div>
	<div class="right">
		<StackTabs {projectId} selectedId={stackId} />
		<div class="branch"></div>
	</div>
</div>

<style>
	.workspace {
		display: flex;
		flex: 1;
		align-items: stretch;
		padding-bottom: 16px;
		padding-right: 16px;
		height: 100%;
		gap: 14px;
	}

	.left {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: flex-start;
		width: 290px;
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
	}

	.left-header {
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 8px 10px 14px;
	}

	.left-body {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.right {
		display: flex;
		flex: 1;
		flex-direction: column;
	}

	.branch {
		border: 1px solid var(--clr-border-2);
		flex: 1;
		border-radius: 0 var(--radius-ml) var(--radius-ml);
	}

	.helper-text {
		text-align: center;
		color: var(--clr-text-2);
		opacity: 0.6;
		margin-top: 10px;
	}
</style>
