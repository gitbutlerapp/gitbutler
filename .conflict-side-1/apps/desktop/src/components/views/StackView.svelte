<script lang="ts">
	import AppScrollableContainer from "$components/shared/AppScrollableContainer.svelte";
	import Resizer from "$components/shared/Resizer.svelte";
	import StackDetails from "$components/views/StackDetails.svelte";
	import StackPanel from "$components/views/StackPanel.svelte";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { sessionChannel } from "$lib/irc/protocol";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { type Stack } from "$lib/stacks/stack";
	import { StackController, setStackContext } from "$lib/stacks/stackController.svelte";
	import { inject } from "@gitbutler/core/context";
	import { persistWithExpiration } from "@gitbutler/shared/persisted";
	import { TestId } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { intersectionObserver } from "@gitbutler/ui/utils/intersectionObserver";

	type Props = {
		projectId: string;
		stack: Stack;
		laneId: string;
		onFoldStack?: () => void;
		onVisible: (visible: boolean) => void;
		clientWidth?: number;
		clientHeight?: number;
	};

	let {
		projectId,
		stack,
		laneId,
		onFoldStack,
		clientHeight = $bindable(),
		clientWidth = $bindable(),
		onVisible,
	}: Props = $props();

	const stackId = $derived(stack.id ?? undefined);
	const topBranchName = $derived(stack.segments.at(0)?.refName?.displayName);
	const segments = $derived(stack.segments);

	const controller = new StackController({
		projectId: () => projectId,
		stackId: () => stackId,
		laneId: () => laneId,
	});
	setStackContext(controller);

	const settingsService = inject(SETTINGS_SERVICE);
	const ircApiService = inject(IRC_API_SERVICE);

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
	const ircEnabled = $derived(
		($settingsStore?.featureFlags?.irc && $settingsStore?.irc?.connection?.enabled) ?? false,
	);

	const ircNickQuery = $derived(ircApiService.nick());
	const ircNick = $derived(ircNickQuery?.response);
	const ircChannel = $derived(
		ircNick && topBranchName ? sessionChannel(ircNick, topBranchName) : undefined,
	);
</script>

<div
	bind:clientWidth
	bind:clientHeight
	data-scrollable-for-dragging
	class="stack-view-wrapper"
	role="presentation"
	class:dimmed={controller.dimmed}
	class:stack-busy={controller.stackBusy}
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
	<AppScrollableContainer childrenWrapHeight="100%" enableDragScroll>
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
			<StackPanel {segments} {topBranchName} {onFoldStack} {ircEnabled} {ircChannel} />

			<!-- RESIZE PANEL 1 -->
			{#if stackViewEl}
				<Resizer
					persistId="ui-stack-width-${controller.stackId}"
					viewport={stackViewEl}
					zIndex="var(--z-lifted)"
					direction="right"
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
	</AppScrollableContainer>

	<!-- DETAILS PANEL -->
	{#if isDetailsOpen}
		<StackDetails {ircChannel} {segments} onWidthChange={updateDetailsViewWidth} />
	{/if}
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

	.dimmed .stack-view,
	.stack-busy .stack-view {
		pointer-events: none;
	}
</style>
