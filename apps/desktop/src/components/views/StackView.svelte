<script lang="ts">
	import ConfigurableScrollableContainer from "$components/shared/ConfigurableScrollableContainer.svelte";
	import FullviewLoading from "$components/shared/FullviewLoading.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import Resizer from "$components/shared/Resizer.svelte";
	import StackDetails from "$components/views/StackDetails.svelte";
	import StackPanel from "$components/views/StackPanel.svelte";
	import { CLAUDE_CODE_SERVICE } from "$lib/codegen/claude";
	import { SETTINGS_SERVICE } from "$lib/config/appSettingsV2";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { sessionChannel } from "$lib/irc/protocol";
	import { IRC_SESSION_BRIDGE } from "$lib/irc/sessionBridge.svelte";
	import { RULES_SERVICE } from "$lib/rules/rulesService.svelte";
	import { StackController, setStackContext } from "$lib/stack/stackController.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { combineResults } from "$lib/state/helpers";
	import { inject } from "@gitbutler/core/context";
	import { persistWithExpiration } from "@gitbutler/shared/persisted";
	import { TestId } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { intersectionObserver } from "@gitbutler/ui/utils/intersectionObserver";
	import { untrack } from "svelte";

	type Props = {
		projectId: string;
		stackId: string | undefined;
		laneId: string;
		onFoldStack?: () => void;
		topBranchName?: string;
		onVisible: (visible: boolean) => void;
		clientWidth?: number;
		clientHeight?: number;
	};

	let {
		projectId,
		stackId,
		laneId,
		onFoldStack,
		topBranchName,
		clientHeight = $bindable(),
		clientWidth = $bindable(),
		onVisible,
	}: Props = $props();

	const controller = new StackController({
		projectId: () => projectId,
		stackId: () => stackId,
		laneId: () => laneId,
	});
	setStackContext(controller);

	const stackService = inject(STACK_SERVICE);
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const rulesService = inject(RULES_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const ircApiService = inject(IRC_API_SERVICE);
	const ircSessionBridge = inject(IRC_SESSION_BRIDGE);

	const branchesQuery = $derived(stackService.branches(controller.projectId, controller.stackId));
	const hasRulesToClear = $derived(
		rulesService.hasRulesToClear(controller.projectId, controller.stackId),
	);
	const claudeConfigQuery = $derived(
		claudeCodeService.claudeConfig({ projectId: controller.projectId }),
	);

	const PANEL1_RESIZER = {
		minWidth: 20,
		maxWidth: 64,
		defaultValue: 23,
	};
	const DETAILS_DEFAULT_WIDTH = 37;

	const persistedStackWidth = $derived(
		persistWithExpiration(
			PANEL1_RESIZER.defaultValue,
			`ui-stack-width-${controller.stackId}`,
			1440,
		),
	);

	let stackViewEl = $state<HTMLDivElement>();

	const isDetailsOpen = $derived(controller.isDetailsViewOpen);

	function updateDetailsViewWidth(width: number) {
		if (stackViewEl) {
			stackViewEl.style.setProperty("--details-view-width", `${width}rem`);
		}
	}

	$effect(() => {
		const element = stackViewEl;
		if (element) {
			if (isDetailsOpen) {
				const currentWidth = element.style.getPropertyValue("--details-view-width");
				if (!currentWidth || currentWidth === "0rem") {
					element.style.setProperty("--details-view-width", `${DETAILS_DEFAULT_WIDTH}rem`);
				}
			} else {
				element.style.setProperty("--details-view-width", "0rem");
			}
		}

		return () => {
			if (element) {
				element.style.removeProperty("--details-view-width");
			}
		};
	});

	const settingsStore = settingsService.appSettings;
	const ircSettings = $derived($settingsStore?.irc);
	const ircEnabled = $derived(
		($settingsStore?.featureFlags?.irc && $settingsStore?.irc?.connection?.enabled) ?? false,
	);

	const ircNickQuery = $derived(ircApiService.nick());
	const ircNick = $derived(ircNickQuery?.response);
	const ircChannel = $derived(
		ircNick && topBranchName ? sessionChannel(ircNick, topBranchName) : undefined,
	);

	const sessionIdQuery = $derived(
		rulesService.aiSessionId(controller.projectId, controller.stackId),
	);
	const sessionId = $derived(sessionIdQuery.response);
	const isManuallyBridged = $derived(ircSessionBridge.isManuallyBridged(stackId));
	const readyToBridge = $derived(
		sessionId &&
			stackId &&
			topBranchName &&
			ircEnabled &&
			(ircSettings?.autoShare || isManuallyBridged.current),
	);

	const connectionStateQuery = $derived(ircEnabled ? ircApiService.connectionState() : undefined);
	const connectionReady = $derived(connectionStateQuery?.response?.ready ?? false);

	$effect(() => {
		if (!readyToBridge) return;
		if (!stackId || !topBranchName) return;

		untrack(() => {
			ircSessionBridge.startBridging({
				projectId,
				stackId,
				branchName: topBranchName,
			});
		});

		return () => {
			ircSessionBridge.stopBridging(stackId);
		};
	});

	$effect(() => {
		if (!stackId) return;
		ircSessionBridge.setBotReady(stackId, connectionReady);
	});
</script>

<div
	bind:clientWidth
	bind:clientHeight
	data-scrollable-for-dragging
	class="stack-view-wrapper"
	role="presentation"
	class:dimmed={controller.dimmed}
	tabindex="-1"
	data-id={controller.stackId}
	data-testid={TestId.Stack}
	data-testid-stackid={controller.stackId}
	data-testid-stack={topBranchName}
	use:intersectionObserver={{
		callback: (entry) => {
			onVisible(!!entry?.isIntersecting);
		},
		options: {
			threshold: 0.5,
		},
	}}
	use:focusable={{
		onKeydown: (event) => {
			if (event.key === "Escape" && isDetailsOpen) {
				controller.closePreview();
				event.preventDefault();
				event.stopPropagation();
				return true;
			}
		},
	}}
>
	<ReduxResult
		projectId={controller.projectId}
		result={combineResults(branchesQuery.result, hasRulesToClear.result, claudeConfigQuery.result)}
	>
		{#snippet loading()}
			<div style:width="{$persistedStackWidth}rem" class="lane-skeleton">
				<FullviewLoading />
			</div>
		{/snippet}
		{#snippet children([branches, hasRulesToClear, claudeConfig])}
			<ConfigurableScrollableContainer childrenWrapHeight="100%" enableDragScroll>
				<div
					class="stack-view"
					class:details-open={isDetailsOpen}
					style:width="{$persistedStackWidth}rem"
					data-fade-on-reorder
					use:focusable={{
						vertical: true,
						onActive: (value) => (controller.active = value),
					}}
					bind:this={stackViewEl}
				>
					<StackPanel {branches} {topBranchName} {onFoldStack} {ircEnabled} {ircChannel} />

					<!-- RESIZE PANEL 1 -->
					{#if stackViewEl}
						<Resizer
							persistId="ui-stack-width-${controller.stackId}"
							viewport={stackViewEl}
							zIndex="var(--z-lifted)"
							direction="right"
							showBorder={!isDetailsOpen}
							minWidth={PANEL1_RESIZER.minWidth}
							maxWidth={PANEL1_RESIZER.maxWidth}
							defaultValue={$persistedStackWidth ?? PANEL1_RESIZER.defaultValue}
							syncName="panel1"
							onWidth={(newWidth) => {
								persistedStackWidth.set(newWidth);
							}}
						/>
					{/if}
				</div>
			</ConfigurableScrollableContainer>

			<!-- DETAILS PANEL -->
			{#if isDetailsOpen}
				<StackDetails
					{hasRulesToClear}
					{claudeConfig}
					{ircChannel}
					onWidthChange={updateDetailsViewWidth}
				/>
			{/if}
		{/snippet}
	</ReduxResult>
</div>

<style lang="postcss">
	.stack-view-wrapper {
		display: flex;
		position: relative;
		flex-shrink: 0;
		height: 100%;
		overflow: hidden;
		transition: opacity 0.15s;

		&.dimmed {
			opacity: 0.5;
		}

		&:focus {
			outline: none;
		}
	}

	.stack-view {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		min-height: 100%;
		padding: 0 12px;
		--details-view-width: 0rem;
	}

	.stack-view.details-open {
		margin-right: calc(var(--details-view-width) + 1.125rem);
	}

	.dimmed .stack-view {
		pointer-events: none;
	}

	.lane-skeleton {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
</style>
