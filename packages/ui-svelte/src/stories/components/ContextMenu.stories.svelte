<script module lang="ts">
	import Button from "$components/Button.svelte";
	import ContextMenu from "$components/ContextMenu.svelte";
	import ContextMenuItem from "$components/ContextMenuItem.svelte";
	import ContextMenuItemSubmenu from "$components/ContextMenuItemSubmenu.svelte";
	import ContextMenuSection from "$components/ContextMenuSection.svelte";
	import { defineMeta } from "@storybook/addon-svelte-csf";

	const { Story } = defineMeta({
		title: "Overlays / ContextMenu",
		args: {
			side: "bottom",
			align: "start",
			offset: 0,
		},
		argTypes: {
			side: {
				control: { type: "select" },
				options: ["top", "bottom", "left", "right"],
				description: "Which side of the trigger element to position the menu",
			},
			align: {
				control: { type: "select" },
				options: ["start", "center", "end"],
				description: "How to align the menu relative to the trigger element",
			},
			offset: {
				control: { type: "number" },
				description: "Distance in pixels from the trigger element",
			},
		},
	});

	let leftClickMenuOpen = $state(false);
	let leftClickTrigger = $state<HTMLButtonElement | undefined>();

	let rightClickMenuOpen = $state(false);
	let rightClickTrigger = $state<HTMLButtonElement | undefined>();
	let rightClickEvent = $state<MouseEvent | undefined>();

	let flickeringMenuOpen = $state(false);
	let flickeringTrigger = $state<HTMLButtonElement | undefined>();

	let submenuMenuOpen = $state(false);
	let submenuTrigger = $state<HTMLButtonElement | undefined>();

	let captionMenuOpen = $state(false);
	let captionTrigger = $state<HTMLButtonElement | undefined>();
</script>

<script lang="ts">
</script>

<Story name="Left click">
	{#snippet template(args)}
		<div class="wrap">
			<Button
				kind="outline"
				bind:el={leftClickTrigger}
				onclick={() => {
					leftClickMenuOpen = !leftClickMenuOpen;
				}}>Toggle context menu</Button
			>
		</div>

		{#if leftClickMenuOpen}
			<ContextMenu
				target={leftClickTrigger}
				{leftClickTrigger}
				onclose={() => {
					leftClickMenuOpen = false;
				}}
				{...args}
			>
				<ContextMenuSection>
					<ContextMenuItem
						label="Commit and bleep"
						icon="commit"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log("Commit and bleep");
							leftClickMenuOpen = false;
						}}
						keyboardShortcut="⌘+Enter"
					/>
					<ContextMenuItem
						label="Commit"
						icon="text-contain"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log("Commit and bleep");
							leftClickMenuOpen = false;
						}}
					/>
				</ContextMenuSection>
				<ContextMenuSection title="More">
					<ContextMenuItem
						label="Another commit"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log("Commit and bleep");
							leftClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Amend"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log("Commit and bleep");
							leftClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Revert"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log("Commit and bleep");
							leftClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Squash"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log("Commit and bleep");
							leftClickMenuOpen = false;
						}}
					/>
				</ContextMenuSection>
				<ContextMenuSection title="Danger zone">
					<ContextMenuItem
						label="Delete"
						onclick={() => {
							// eslint-disable-next-line no-console
							console.log("Commit and bleep");
							leftClickMenuOpen = false;
						}}
					/>
				</ContextMenuSection>
			</ContextMenu>
		{/if}
	{/snippet}
</Story>

<!-- eslint-disable no-console -->
<Story name="Right-click">
	{#snippet template(args)}
		<div class="wrap">
			<Button
				kind="outline"
				bind:el={rightClickTrigger}
				oncontextmenu={(e) => {
					e.preventDefault();
					rightClickEvent = e;
					rightClickMenuOpen = !rightClickMenuOpen;
				}}>Right-click for scrollable menu</Button
			>
		</div>

		{#if rightClickMenuOpen}
			<ContextMenu
				target={rightClickEvent}
				{rightClickTrigger}
				onclose={() => {
					rightClickMenuOpen = false;
				}}
				{...args}
			>
				<ContextMenuSection title="Quick actions">
					<ContextMenuItem
						label="Commit"
						onclick={() => {
							console.log("Commit");
							rightClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Push"
						onclick={() => {
							console.log("Push");
							rightClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Pull"
						onclick={() => {
							console.log("Pull");
							rightClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Fetch"
						onclick={() => {
							console.log("Fetch");
							rightClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Merge"
						onclick={() => {
							console.log("Merge");
							rightClickMenuOpen = false;
						}}
					/>
					<ContextMenuItemSubmenu label="More actions">
						{#snippet submenu({ close })}
							<ContextMenuSection>
								<ContextMenuItem
									label="Cherry-pick"
									onclick={() => {
										console.log("Cherry-pick");
										close();
										rightClickMenuOpen = false;
									}}
								/>
								<ContextMenuItem
									label="Rebase"
									onclick={() => {
										console.log("Rebase");
										close();
										rightClickMenuOpen = false;
									}}
								/>
							</ContextMenuSection>
						{/snippet}
					</ContextMenuItemSubmenu>
				</ContextMenuSection>
				<ContextMenuSection title="Branch operations">
					<ContextMenuItem
						label="Create branch"
						onclick={() => {
							console.log("Create branch");
							rightClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Switch branch"
						onclick={() => {
							console.log("Switch branch");
							rightClickMenuOpen = false;
						}}
					/>
				</ContextMenuSection>
				<ContextMenuSection>
					<ContextMenuItem
						label="Stage changes"
						onclick={() => {
							console.log("Stage changes");
							rightClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Unstage changes"
						onclick={() => {
							console.log("Unstage changes");
							rightClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Discard changes"
						onclick={() => {
							console.log("Discard changes");
							rightClickMenuOpen = false;
						}}
					/>
				</ContextMenuSection>
				<ContextMenuSection>
					<ContextMenuItem
						label="Bisect"
						onclick={() => {
							console.log("Bisect");
							rightClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Reflog"
						onclick={() => {
							console.log("Reflog");
							rightClickMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Worktree"
						onclick={() => {
							console.log("Worktree");
							rightClickMenuOpen = false;
						}}
					/>
				</ContextMenuSection>
			</ContextMenu>
		{/if}
	{/snippet}
</Story>

<!-- eslint-disable no-console -->
<Story name="Bottom positioned (test flickering fix)">
	{#snippet template(args)}
		<div class="wrap" style="margin-top: 200px;">
			<Button
				kind="outline"
				bind:el={flickeringTrigger}
				onclick={() => {
					flickeringMenuOpen = !flickeringMenuOpen;
				}}>Test flickering fix</Button
			>
		</div>

		{#if flickeringMenuOpen}
			<ContextMenu
				target={flickeringTrigger}
				leftClickTrigger={flickeringTrigger}
				side="bottom"
				onclose={() => {
					flickeringMenuOpen = false;
				}}
				{...args}
			>
				<ContextMenuSection title="Many items to force scrolling">
					<ContextMenuItem
						label="Item 1"
						onclick={() => {
							console.log("Item 1");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 2"
						onclick={() => {
							console.log("Item 2");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 3"
						onclick={() => {
							console.log("Item 3");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 4"
						onclick={() => {
							console.log("Item 4");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 5"
						onclick={() => {
							console.log("Item 5");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 6"
						onclick={() => {
							console.log("Item 6");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 7"
						onclick={() => {
							console.log("Item 7");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 8"
						onclick={() => {
							console.log("Item 8");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 9"
						onclick={() => {
							console.log("Item 9");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 10"
						onclick={() => {
							console.log("Item 10");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 11"
						onclick={() => {
							console.log("Item 11");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 12"
						onclick={() => {
							console.log("Item 12");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 13"
						onclick={() => {
							console.log("Item 13");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 14"
						onclick={() => {
							console.log("Item 14");
							flickeringMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Item 15"
						onclick={() => {
							console.log("Item 15");
							flickeringMenuOpen = false;
						}}
					/>
				</ContextMenuSection>
			</ContextMenu>
		{/if}
	{/snippet}
</Story>

<Story name="With Submenus">
	{#snippet template(args)}
		<div class="wrap">
			<Button
				kind="outline"
				bind:el={submenuTrigger}
				onclick={() => {
					submenuMenuOpen = !submenuMenuOpen;
				}}>Context menu with submenus</Button
			>
		</div>

		{#if submenuMenuOpen}
			<ContextMenu
				target={submenuTrigger}
				leftClickTrigger={submenuTrigger}
				side="bottom"
				onclose={() => {
					submenuMenuOpen = false;
				}}
				{...args}
			>
				<ContextMenuSection title="Basic actions">
					<ContextMenuItem
						label="Copy"
						onclick={() => {
							console.log("Copy");
							submenuMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Paste"
						onclick={() => {
							console.log("Paste");
							submenuMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Cut"
						onclick={() => {
							console.log("Cut");
							submenuMenuOpen = false;
						}}
					/>
				</ContextMenuSection>
				<ContextMenuSection title="Advanced">
					<ContextMenuItemSubmenu label="Format" icon="text-block">
						{#snippet submenu({ close })}
							<ContextMenuSection>
								<ContextMenuItem
									label="Bold"
									onclick={() => {
										console.log("Bold");
										close();
										submenuMenuOpen = false;
									}}
								/>
								<ContextMenuItem
									label="Italic"
									onclick={() => {
										console.log("Italic");
										close();
										submenuMenuOpen = false;
									}}
								/>
								<ContextMenuItem
									label="Underline"
									onclick={() => {
										console.log("Underline");
										close();
										submenuMenuOpen = false;
									}}
								/>
							</ContextMenuSection>
							<ContextMenuSection title="Alignment">
								<ContextMenuItem
									label="Left"
									onclick={() => {
										console.log("Left");
										close();
										submenuMenuOpen = false;
									}}
								/>
								<ContextMenuItem
									label="Center"
									onclick={() => {
										console.log("Center");
										close();
										submenuMenuOpen = false;
									}}
								/>
								<ContextMenuItem
									label="Right"
									onclick={() => {
										console.log("Right");
										close();
										submenuMenuOpen = false;
									}}
								/>
							</ContextMenuSection>
						{/snippet}
					</ContextMenuItemSubmenu>
					<ContextMenuItemSubmenu label="Insert" icon="plus">
						{#snippet submenu({ close })}
							<ContextMenuSection>
								<ContextMenuItem
									label="Image"
									onclick={() => {
										console.log("Image");
										close();
										submenuMenuOpen = false;
									}}
								/>
								<ContextMenuItem
									label="Link"
									onclick={() => {
										console.log("Link");
										close();
										submenuMenuOpen = false;
									}}
								/>
								<ContextMenuItem
									label="Table"
									onclick={() => {
										console.log("Table");
										close();
										submenuMenuOpen = false;
									}}
								/>
							</ContextMenuSection>
						{/snippet}
					</ContextMenuItemSubmenu>
				</ContextMenuSection>
			</ContextMenu>
		{/if}
	{/snippet}
</Story>

<Story name="With caption">
	{#snippet template(args)}
		<div class="wrap">
			<Button
				kind="outline"
				bind:el={captionTrigger}
				onclick={() => {
					captionMenuOpen = !captionMenuOpen;
				}}>Context menu with caption</Button
			>
		</div>

		{#if captionMenuOpen}
			<ContextMenu
				target={captionTrigger}
				leftClickTrigger={captionTrigger}
				side="bottom"
				onclose={() => {
					captionMenuOpen = false;
				}}
				{...args}
			>
				<ContextMenuSection title="Item with caption">
					<ContextMenuItem
						label="Rebase"
						caption="Move your commits on top of upstream changes. Creates clean, linear history."
						onclick={() => {
							console.log("Copy");
							captionMenuOpen = false;
						}}
					/>
					<ContextMenuItem
						label="Paste"
						caption="Paste from clipboard"
						onclick={() => {
							console.log("Paste");
							captionMenuOpen = false;
						}}
					/>
				</ContextMenuSection>
			</ContextMenu>
		{/if}
	{/snippet}
</Story>

<style>
	.wrap {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		padding: 400px 0 100px;
		border-radius: var(--radius-ml);
		background: var(--bg-2);
	}
</style>
