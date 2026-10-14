import preview from "#storybook/preview";
import type { FC } from "react";
import { Button } from "./Button.tsx";
import { Toast } from "@base-ui/react";
import { Snackbars, type SnackbarData, type SnackbarType } from "./Snackbars.tsx";

type Sample = {
	type: SnackbarType;
	title: string;
	data?: SnackbarData;
	/** What a `loading` one turns into once the work is done. */
	result?: { type: SnackbarType; title: string; data?: SnackbarData };
};

const samples: Array<Sample> = [
	{
		type: "loading",
		title: "Rewording on macbook…",
		result: { type: "safe", title: "Commit reworded" },
	},
	{
		type: "loading",
		title: "Fetching remotes on pavel--macbook-pro-2…",
		result: { type: "safe", title: "Remotes fetched on pavel--macbook-pro-2" },
	},
	{
		type: "loading",
		title: "Absorbing…",
		result: { type: "safe", title: "Changes absorbed into 3 commits", data: { icon: "commit" } },
	},
	{ type: "safe", title: "Branch name copied" },
	{ type: "safe", title: "bench-tweaks landed on main" },
	{ type: "danger", title: "Couldn’t copy the diff" },
	{ type: "danger", title: "The diff is too large to copy from here" },
	{ type: "warning", title: "Only 2 of 3 files were absorbed" },
	{
		type: "info",
		title: "Couldn’t work out where to absorb",
		data: { icon: "absorb", dismissable: true },
	},
];

const manager = Toast.createToastManager<SnackbarData>();

const show = () => {
	const sample = samples[Math.floor(Math.random() * samples.length)];
	if (sample === undefined) return;
	const { result, ...first } = sample;
	const id = manager.add({ ...first, timeout: first.data?.dismissable ? 0 : undefined });
	// The work behind a busy one finishes a moment later; its result takes the timer from there.
	if (result) setTimeout(() => manager.update(id, { ...result, data: result.data ?? {} }), 1500);
};

const Playground: FC = () => (
	<>
		<Button onClick={show}>Show a snackbar</Button>
		<Snackbars manager={manager} />
	</>
);

const meta = preview.meta({
	title: "Overlays/Snackbars",
	id: "components-snackbars",
	component: Snackbars,
	args: { manager },
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=1706-1682",
		},
	},
	render: () => <Playground />,
});

/**
 * The stack at the bottom centre, one below another with the newest at the bottom. Each press raises
 * a random snackbar: a busy one turns into its result, one on a timer sinks out after a few seconds,
 * and a click on any lets it go early.
 */
export const Default = meta.story({});
