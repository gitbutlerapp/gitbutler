<script lang="ts">
	import IntegrateUpstreamWorkspaceModal from "$components/upstream/IntegrateUpstreamWorkspaceModal.svelte";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { URL_SERVICE } from "$lib/backend/url";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { WORKSPACE_UPSTREAM_INTEGRATION_SERVICE } from "$lib/upstream/workspaceUpstreamIntegrationService.svelte";
	import { provideAll } from "@gitbutler/core/context";
	import type { BaseBranch, RefInfo } from "@gitbutler/but-sdk";

	type Props = {
		base: BaseBranch;
		currentHeadInfo: RefInfo;
		previewHeadInfo?: RefInfo;
		previewError?: string;
		onIntegrate?: () => Promise<unknown>;
	};

	let { base, currentHeadInfo, previewHeadInfo, previewError, onIntegrate }: Props = $props();

	provideAll([
		[
			BASE_BRANCH_SERVICE,
			{
				baseBranch: () => ({ response: base }),
				refreshBaseBranch: async () => {},
			},
		],
		[
			WORKSPACE_UPSTREAM_INTEGRATION_SERVICE,
			{
				headInfo: () => ({ response: currentHeadInfo }),
				fetchHeadInfo: async () => currentHeadInfo,
				preview: async () => {
					if (previewError) throw new Error(previewError);
					return { replacedCommits: {}, headInfo: previewHeadInfo ?? currentHeadInfo };
				},
				integrateUpstream: () => [onIntegrate ?? (async () => ({ replacedCommits: {}, headInfo: previewHeadInfo ?? currentHeadInfo })), {}],
				deleteLocalBranch: async () => {},
			},
		],
		[
			STACK_SERVICE,
			{
				unapply: async () => {},
				deleteLocalBranch: async () => {},
			},
		],
		[DEFAULT_FORGE_FACTORY, { current: { commitUrl: () => undefined } }],
		[URL_SERVICE, { openExternalUrl: async () => {} }],
		[CLIPBOARD_SERVICE, { write: async () => {} }],
	]);

	let modal = $state<{ show: () => Promise<void> }>();

	$effect(() => {
		modal?.show();
	});
</script>

<IntegrateUpstreamWorkspaceModal bind:this={modal} projectId="project-id" />
