import preview from "#storybook/preview";
import { PickerDialog, type PickerDialogGroup } from "./PickerDialog.tsx";

type Branch = { ref: string; name: string; updated: string };

const groups: Array<PickerDialogGroup<Branch>> = [
	{
		value: "Local",
		items: [
			{ ref: "refs/heads/toolbox-gap", name: "toolbox-gap", updated: "2 hours ago" },
			{ ref: "refs/heads/design-pages", name: "design-pages", updated: "yesterday" },
			{ ref: "refs/heads/panel-behind", name: "panel-behind", updated: "3 days ago" },
		],
	},
	{
		value: "origin",
		items: [
			{ ref: "refs/remotes/origin/askpass-modal", name: "askpass-modal", updated: "last week" },
			{
				ref: "refs/remotes/origin/hunk-separators",
				name: "hunk-separators",
				updated: "2 weeks ago",
			},
		],
	},
];

const meta = preview.meta({
	// The bare name, not `PickerDialog<Branch>`: the manifest resolves the component by it. The
	// args' item is then `unknown`, so each callback names its type.
	component: PickerDialog,
	args: {
		ariaLabel: "Apply branch",
		closeLabel: "Close apply branch picker",
		nothingFoundLabel: "No available branches found",
		nothingToListLabel: "Nothing left to apply",
		placeholder: "Search for branches to apply…",
		selectLabel: "Apply branch",
		getItemKey: (branch) => (branch as Branch).ref,
		getItemLabel: (branch) => (branch as Branch).name,
		getItemType: (branch) => (branch as Branch).updated,
		items: groups,
		open: true,
		onOpenChange: () => {},
		onSelectItem: () => {},
	},
});

/** Grouped items, filtered as you type; Enter runs the named action on the highlighted one. */
export const Default = meta.story({});

/** Nothing in the list before anything was typed. */
export const NothingToList = meta.story({
	args: { items: [] },
});
