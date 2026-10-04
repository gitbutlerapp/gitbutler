import { useQuery } from "@tanstack/react-query";
import { type FC, useId } from "react";
import type { Commit, HostedMachine, MachineBranch } from "@gitbutler/but-sdk";
import { commitAddress } from "#ui/addresses.ts";
import { hostedMachinesQueryOptions, hostedPresenceQueryOptions } from "#ui/api/queries.ts";
import { useHostedBranchPull } from "#ui/api/mutations.ts";
import { commitBody, commitTitle } from "#ui/commit.ts";
import { GraphSegment } from "#ui/components/GraphSegment.tsx";
import {
	type NativeMenuItem,
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
} from "#ui/native-menu.ts";
import { useHostedSync } from "#ui/HostedSync.tsx";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { setCursor } from "#ui/use-cursor.ts";
import { Toolbar } from "@base-ui/react";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { Tooltip } from "@gitbutler/ui-react/Tooltip.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { RelativeTime } from "@gitbutler/ui-react/RelativeTime.tsx";
import { BranchRowHeadline } from "../BranchRowHeadline.tsx";
import { CommitRowContent } from "../CommitRowContent.tsx";
import {
	Row,
	RowLabel,
	RowLabelContainer,
	RowLabelGroup,
	RowMeta,
	RowToolbar,
	SectionHeaderRow,
} from "../Row.tsx";
import rowStyles from "../Row.module.css";
import { getRowButtonClassName, useIsSelected } from "../Row-utils.ts";
import { StackCard } from "../StackCard.tsx";
import { machineFoldKey } from "./fold.ts";
import { TreeItem } from "./TreeItem.tsx";
import styles from "./RemoteMachines.module.css";

/** Your other machines that published to the hosted server, below this machine's workspace. */
export const RemoteMachines: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: published = [] } = useQuery(hostedMachinesQueryOptions(projectId));
	const { data: online = [] } = useQuery(hostedPresenceQueryOptions(projectId));
	// A machine that's online is listed whether or not it published anything yet.
	const machines: Array<HostedMachine> = [
		...published,
		...online
			.filter((name) => !published.some((machine) => machine.name === name))
			.map((name) => ({ name, publishedAt: 0, branches: [] })),
	];
	if (machines.length === 0) return null;

	return (
		<div className={styles.machines}>
			{machines.map((machine) => (
				<Machine
					key={machine.name}
					projectId={projectId}
					machine={machine}
					online={online.includes(machine.name)}
				/>
			))}
		</div>
	);
};

const Machine: FC<{ projectId: string; machine: HostedMachine; online: boolean }> = ({
	projectId,
	machine,
	online,
}) => {
	const dispatch = useAppDispatch();
	// Beside the workspace's folds, so the address space leaves a folded machine's commits out.
	const folded = useAppSelector(
		(state) =>
			projectSlice.selectors.selectFoldedSegments(state, projectId)[
				machineFoldKey(machine.name)
			] === true,
	);

	return (
		<>
			<SectionHeaderRow
				label={
					<>
						<span aria-hidden className={classes(styles.presence, online && styles.online)} />
						{machine.name}
						<span className={styles.hidden}>{online ? ", online" : ", offline"}</span>
					</>
				}
				className={styles.header}
				leading={
					<button
						type="button"
						aria-expanded={!folded}
						aria-label={folded ? `Unfold ${machine.name}` : `Fold ${machine.name}`}
						className={getRowButtonClassName({ iconOnly: true })}
						onClick={() =>
							dispatch(
								projectSlice.actions.toggleSegmentFolded({
									projectId,
									branchRef: machineFoldKey(machine.name),
								}),
							)
						}
					>
						<Icon name={folded ? "chevron-right" : "chevron-down"} />
					</button>
				}
				actions={
					machine.publishedAt > 0 && (
						<span className={styles.when}>
							<RelativeTime timestamp={machine.publishedAt} compact />
						</span>
					)
				}
			/>
			{!folded &&
				(machine.branches.length === 0 ? (
					<Row interactive={false}>
						<RowLabelContainer>
							<RowLabel className={rowStyles.fadedText}>Nothing published yet</RowLabel>
						</RowLabelContainer>
					</Row>
				) : (
					machine.branches.map((branch) => (
						<RemoteBranch
							key={branch.branch}
							projectId={projectId}
							machine={machine.name}
							branch={branch}
						/>
					))
				))}
		</>
	);
};

/** A branch as another machine last sent it, drawn as a stack is, with only pulling to do. */
const RemoteBranch: FC<{ projectId: string; machine: string; branch: MachineBranch }> = ({
	projectId,
	machine,
	branch,
}) => {
	const { mutate, isPending } = useHostedBranchPull(projectId);
	const settle = useHostedSync();
	const { local } = branch;

	const pull = (into: "worktree" | "workspace", overwrite = false) =>
		mutate(
			{
				projectId,
				machine,
				branch: branch.branch,
				intoWorkspace: into === "workspace",
				onConflict: overwrite ? "overwrite" : null,
			},
			{
				onSuccess: (outcome) =>
					settle(outcome, {
						title: "Overwrite local work?",
						keepLabel: "Keep local",
						overwrite: () => pull(into, true),
					}),
			},
		);

	// The hosted page only reads; pulling is for machines. Where a branch already lives, a
	// pull updates it there.
	const canPull = window.lite.hosted !== true;
	const into = local.type === "workspace" ? "workspace" : "worktree";
	const pullHint =
		local.type === "worktree"
			? "Pull into its worktree"
			: local.type === "workspace"
				? "Pull into the workspace"
				: "Pull into a new worktree";
	const pullItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Pull into Worktree",
			enabled: !isPending && local.type !== "workspace",
			onSelect: () => pull("worktree"),
		}),
		nativeMenuItem({
			label: "Pull into Workspace",
			enabled: !isPending && local.type !== "worktree",
			onSelect: () => pull("workspace"),
		}),
		nativeMenuSeparator,
	];
	const menuItems: Array<NativeMenuItem> = [
		...(canPull ? pullItems : []),
		nativeMenuItem({
			label: "Copy Branch Name",
			onSelect: () => window.lite.clipboardWriteText(branch.branch),
		}),
	];

	// The snapshot commit holds the uncommitted changes, on top of the branch's own.
	const rows = branch.uncommitted ? [branch.uncommitted, ...branch.commits] : branch.commits;

	return (
		// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A stack is an ARIA group of tree items.
		<StackCard role="group" aria-label={branch.branch} className={styles.card}>
			<Row
				interactive={false}
				onContextMenu={(event) => void showNativeContextMenu(event, menuItems)}
			>
				<GraphSegment glyph="forkRight" status="LocalAndRemote" />
				<RowLabelGroup>
					<BranchRowHeadline title={branch.branch} />
					<RowMeta>
						{branch.uncommitted ? "With uncommitted changes" : "Committed changes only"}
					</RowMeta>
				</RowLabelGroup>
				{canPull && (
					<Tooltip content={pullHint}>
						<Button size="small" disabled={isPending} onClick={() => pull(into)}>
							Pull
						</Button>
					</Tooltip>
				)}
				<RowMenuButton label="Branch menu" items={menuItems} />
			</Row>
			{rows.map((commit, index) => (
				<RemoteCommit
					key={commit.id}
					commit={commit}
					uncommitted={commit === branch.uncommitted}
					positionInSet={index + 1}
					setSize={rows.length}
				/>
			))}
		</StackCard>
	);
};

/**
 * A commit another machine published: selectable like the workspace's, with nothing to change.
 * Its uncommitted changes are a commit too, shown as what they are.
 */
const RemoteCommit: FC<{
	commit: Commit;
	uncommitted: boolean;
	positionInSet: number;
	setSize: number;
}> = ({ commit, uncommitted, positionInSet, setSize }) => {
	const address = commitAddress({ commitId: commit.id, changeId: commit.changeId });
	const isSelected = useIsSelected(address, "applied");
	const descriptionId = useId();
	const title = uncommitted ? undefined : commitTitle(commit.message);
	const body = uncommitted ? undefined : commitBody(commit.message);
	// What a commit outside the workspace offers: reading it, never changing it.
	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Copy",
			submenu: [
				nativeMenuItem({
					label: "Change ID",
					enabled: commit.changeId !== "",
					onSelect: () => window.lite.clipboardWriteText(commit.changeId),
				}),
				nativeMenuItem({
					label: "Commit ID",
					onSelect: () => window.lite.clipboardWriteText(commit.id),
				}),
				nativeMenuItem({
					label: "Commit Title",
					enabled: title !== undefined,
					onSelect: () => window.lite.clipboardWriteText(title ?? ""),
				}),
				nativeMenuItem({
					label: "Commit Body",
					enabled: body !== undefined,
					onSelect: () => window.lite.clipboardWriteText(body ?? ""),
				}),
			],
		}),
	];

	return (
		<TreeItem
			address={address}
			aria-label={uncommitted ? "Uncommitted changes" : (title ?? "(no message)")}
			aria-describedby={uncommitted ? undefined : descriptionId}
			aria-level={2}
			aria-posinset={positionInSet}
			aria-setsize={setSize}
		>
			<Row
				isSelected={isSelected}
				onSelect={() => setCursor("applied", address)}
				onContextMenu={
					uncommitted ? undefined : (event) => void showNativeContextMenu(event, menuItems)
				}
			>
				{uncommitted ? (
					<>
						<GraphSegment glyph="parent" status={commit.state.type} />
						<RowLabelContainer>
							<RowLabel className={rowStyles.fadedText}>Uncommitted changes</RowLabel>
						</RowLabelContainer>
					</>
				) : (
					<>
						<GraphSegment glyph="commit" status={commit.state.type} />
						<CommitRowContent
							commit={commit}
							hasConflicts={commit.hasConflicts}
							descriptionId={descriptionId}
						/>
						<RowMenuButton label="Commit menu" items={menuItems} />
					</>
				)}
			</Row>
		</TreeItem>
	);
};

/** The kebab every row offers its menu behind, as the workspace's rows do. */
const RowMenuButton: FC<{ label: string; items: Array<NativeMenuItem> }> = ({ label, items }) => (
	<Toolbar.Root aria-label={label} render={<RowToolbar reserveSpace />}>
		<Toolbar.Button
			aria-label={label}
			onClick={(event) => void showNativeMenuFromTrigger(event.currentTarget, items)}
			className={getRowButtonClassName({ iconOnly: true })}
		>
			<Icon name="kebab" />
		</Toolbar.Button>
	</Toolbar.Root>
);
