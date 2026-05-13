/* eslint-disable */

import { changesSectionOperand, Operand } from "#ui/operands.ts";
import { AppDispatch } from "#ui/store.ts";
import { Hotkey, useHotkey, useHotkeys } from "@tanstack/react-hotkeys";
import { projectActions } from "#ui/projects/state.ts";
import { focusPanel } from "#ui/panels.ts";
import { FC, ReactNode } from "react";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { keyboardTransferOperationMode } from "#ui/outline/mode.ts";

// "selection" is the general menu invoked by cmd+k
type PaletteView = "selection" | "commit.add_empty";

// Views determine the hotkeys that are available. These can be as wide or granular as we'd like.
// They don't necessarily have to align 1:1 with what we call panels.
type View = "Details" | "Files" | "Outline";

type CommandId =
	| "commit.add_empty"
	| "commit.add_empty.above"
	| "commit.add_empty.below"
	| "commit.amend";

// we could optionally take a param here (like we do in real useCommand now) but, like there, needs
// to be consistent across all command functions
type CommandFn = () => void;

declare const useCommands: () => [
	Record<CommandId, CommandFn>,
	Partial<Record<CommandId, Array<Hotkey>>>,
];

const useCommand = (cid: CommandId): [CommandFn, Array<Hotkey>] => {
	const [cmds, hotkeys] = useCommands();
	return [cmds[cid], hotkeys[cid] ?? []];
};

// deps that almost all commands need, we'll supply once and then abstract behind a hook
// unclear how each cmd could take its own data if that data is local... and if it's global, we
// already have it
type Deps = {
	dispatch: AppDispatch;
	projectId: string;
};

const getCommands = ({ dispatch, projectId }: Deps): Record<CommandId, CommandFn> => {
	const gotoPaletteView = (pv: PaletteView): void => {
		console.log("GO TO palette view", pv);
	};

	return {
		"commit.add_empty": () => gotoPaletteView("commit.add_empty"),
		"commit.add_empty.above": () => console.log("TODO"),
		"commit.add_empty.below": () => console.log("TODO"),
		"commit.amend": () => {
			dispatch(
				projectActions.enterTransferMode({
					projectId,
					// TODO is this modelled correctly? on master
					mode: keyboardTransferOperationMode({
						source: changesSectionOperand,
						operationType: "rub",
					}),
				}),
			);

			focusPanel("outline");
		},
	};
};

const hotkeys = (view: View): Partial<Record<CommandId, Array<Hotkey>>> => {
	switch (view) {
		case "Outline":
			// TODO: Static, could/should extract.
			return { "commit.amend": ["Shift+A"] };
		default:
			return {};
	}
};

// for shortcuts bar, cheatsheet, etc. will need to map from all command ids to whatever label they
// want to render
const allHotkeys = (view: View): Array<[CommandId, Array<Hotkey>]> =>
	Object.entries(hotkeys(view)) as Array<[CommandId, Array<Hotkey>]>; // annoying inference

// labels are inlined locally since they aren't necessarily reused. likewise right click context etc
// is inlined albeit can reuse command fns and hotkeys

// hotkey is dynamically rendered based upon the view
// we can potentially add other static data here like an icon
type Item = {
	// trailing ... is dynamically rendered based upon the command opening a new palette view
	label: ReactNode;
	commandId: CommandId;
};

// usage examples

// // app root
const App: FC<{ selection: Operand; view: View; paletteView: PaletteView }> = (p) => {
	const [cmds] = useCommands();
	const hotkeysByCommand = allHotkeys(p.view);

	// shouldn't have conflict issues as hotkeys defined not to ever conflict in the same view -
	// enabled should have no bearing
	useHotkeys(
		hotkeysByCommand.flatMap(([cid, hotkeys]) =>
			hotkeys.map((hotkey) => ({
				hotkey,
				callback: cmds[cid],
			})),
		),
	);

	return (
		// redux provider, rest of app, etc
		<>
			<div>
				<MyButton />
			</div>

			<Palette selection={p.selection} paletteView={p.paletteView} />
		</>
	);
};

// // palette
// // These contextually appear and require data/context to do so. Multiple may appear.
// type SelectionGroup = "Branch" | "Commit" | "Stack";

// // These always appear, and always in the same order. Grouping is arbitrary and the requisite data
// // is (effectively) global.
// type GeneralGroup = "Branches" | "Changes" | "Details" | "Project" | "Outline";
const Palette: FC<{ selection: Operand; paletteView: PaletteView }> = (p) => {
	switch (p.paletteView) {
		case "selection":
			return (
				<ul>
					{"commitId" in p.selection && (
						<Group label="Commit">
							<Item commandId="commit.amend">Amend commit</Item>
							<Item commandId="commit.add_empty">Add empty commit...</Item>
						</Group>
					)}

					{"branchRef" in p.selection && <Group label="Branch" />}

					{"stackId" in p.selection && <Group label="Stack" />}

					<Group label="Example" />
				</ul>
			);

		case "commit.add_empty":
			return (
				<ul>
					<Item commandId="commit.add_empty.above">Add empty commit above</Item>
					<Item commandId="commit.add_empty.below">Add empty commit below</Item>
				</ul>
			);
	}
};

const Group: FC<{ label: ReactNode; children?: ReactNode }> = (p) => {
	return (
		<ul>
			<h1>{p.label}</h1>

			<div>{p.children}</div>
		</ul>
	);
};

const Item = <T extends CommandId>(p: { commandId: T; children?: ReactNode }): ReactNode => {
	const [cmds, hotkeys] = useCommands();

	return (
		<li onClick={cmds[p.commandId]}>
			{p.children} {hotkeys[p.commandId]?.join(", ")}
		</li>
	);
};

// // button + hotkey
const MyButton: FC = () => {
	// these should only be navigational so shouldn't conflict with global hotkeys in practice. not
	// 100% sure (e.g. a/b for above/below in context tooltip - or is that a command?)
	useHotkey("ArrowDown", () => console.log("example non-command hotkey"));

	const [amendCommit, amendCommitHotkeys] = useCommand("commit.amend");

	return (
		<ShortcutButton onClick={amendCommit} hotkeys={amendCommitHotkeys}>
			Amend commit
		</ShortcutButton>
	);
};
