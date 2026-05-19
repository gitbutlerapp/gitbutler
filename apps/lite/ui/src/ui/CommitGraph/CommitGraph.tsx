import { FC } from "react";
import type { CommitGraphRow } from "./commitGraphRows.ts";
import styles from "./CommitGraph.module.css";

type Props = {
	rows: Array<CommitGraphRow>;
};

const shortCommitId = (commitId: string): string => commitId.slice(0, 7);

const renderRefs = (refs: Array<string>) =>
	refs.length === 0 ? null : (
		<span className={styles.refs}>
			{" ("}
			{refs.join(", ")}
			{")"}
		</span>
	);

export const CommitGraph: FC<Props> = ({ rows }) => (
	<div className={styles.graph}>
		{rows.map((row, index) => {
			const key = row.kind === "commit" ? row.content.commitId : `join-${index}`;

			return (
				<div className={styles.row} key={key}>
					<LeftRail row={row} />
					<NodeComponent row={row} />
					<RightRail row={row} />
					<div className={styles.content}>
						{row.kind === "commit" ? (
							<>
								<span className={styles.sha}>{shortCommitId(row.content.commitId)}</span>
								{renderRefs(row.content.refs)}{" "}
								<span className={styles.subject}>{row.content.subject}</span>
							</>
						) : null}
					</div>
				</div>
			);
		})}
	</div>
);

interface LeftRailProps {
	row: CommitGraphRow;
}

function LeftRail(props: LeftRailProps) {
	if (props.row.leftRail === "|")
		return (
			<div className={styles["vertical-edge-container"]}>
				<div className={styles["vertical-edge"]} />
			</div>
		);

	return null;
}

interface RightRailProps {
	row: CommitGraphRow;
}

function RightRail(props: RightRailProps) {
	return <span className={styles.cell}>{props.row.rightRail}</span>;
}

interface NodeProps {
	row: CommitGraphRow;
}

function NodeComponent(props: NodeProps) {
	if (props.row.node === "*")
		return (
			<div className={styles["node-container"]}>
				<div className={styles.node} />
			</div>
		);

	return <span className={styles.cell}>{props.row.node}</span>;
}
