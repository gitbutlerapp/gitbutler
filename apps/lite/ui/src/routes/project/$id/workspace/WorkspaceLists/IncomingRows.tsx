import rowStyles from "../Row.module.css";
import { decodeBytes } from "#ui/api/bytes.ts";
import { branchAddress, addressIdentityKey } from "#ui/addresses.ts";
import { GraphSegment } from "#ui/components/GraphSegment.tsx";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { addressSpaceIncludes } from "#ui/workspace/address-space.ts";
import type { BranchReference, Segment } from "@gitbutler/but-sdk";
import type { FC } from "react";
import { Row } from "../Row.tsx";
import { CommitRowContent } from "../CommitRowContent.tsx";
import { useAddressSpace } from "./context.tsx";

/**
 * The remote's commits the branch lacks, opened from the branch row's chip:
 * ghosted rows on the branch's own rail. Not in the address space, so they
 * dim with the branch like its other addressless rows.
 */
export const IncomingRows: FC<{
	projectId: string;
	segment: Segment;
	refName: BranchReference;
	/** Columns of the main line running behind the rows, left of the branch's rail. */
	behind: number;
}> = ({ projectId, segment, refName, behind }) => {
	const addressSpace = useAddressSpace();
	const branchRef = decodeBytes(refName.fullNameBytes);
	const expanded = useAppSelector((state) =>
		projectSlice.selectors.selectIncomingExpanded(state, projectId, branchRef),
	);
	if (!expanded) return null;

	const inert = !addressSpaceIncludes(
		addressSpace,
		branchAddress({ branchRef: refName.fullNameBytes }),
		addressIdentityKey,
	);

	return segment.commitsOnRemote.map((commit) => (
		<Row key={commit.id} interactive={false} inert={inert}>
			<GraphSegment glyph="commit" status="Upstream" behind={behind} />
			<CommitRowContent commit={commit} className={rowStyles.fadedText} />
		</Row>
	));
};
