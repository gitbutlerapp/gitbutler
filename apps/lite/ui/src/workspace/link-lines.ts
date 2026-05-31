import type { LinkLine } from "@gitbutler/but-sdk";

/** This cell contains a horizontal line that connects to a parent. */
export const HORIZ_PARENT: LinkLine = 0b0_0000_0000_0001;

/** This cell contains a horizontal line that connects to an ancestor. */
export const HORIZ_ANCESTOR: LinkLine = 0b0_0000_0000_0010;

/** The descendent of this cell is connected to the parent. */
export const VERT_PARENT: LinkLine = 0b0_0000_0000_0100;

/** The descendent of this cell is connected to an ancestor. */
export const VERT_ANCESTOR: LinkLine = 0b0_0000_0000_1000;

/**
 * The parent of this cell is linked in this link row and the child is to the
 * left.
 */
export const LEFT_FORK_PARENT: LinkLine = 0b0_0000_0001_0000;

/**
 * The ancestor of this cell is linked in this link row and the child is to the
 * left.
 */
export const LEFT_FORK_ANCESTOR: LinkLine = 0b0_0000_0010_0000;

/**
 * The parent of this cell is linked in this link row and the child is to the
 * right.
 */
export const RIGHT_FORK_PARENT: LinkLine = 0b0_0000_0100_0000;

/**
 * The ancestor of this cell is linked in this link row and the child is to the
 * right.
 */
export const RIGHT_FORK_ANCESTOR: LinkLine = 0b0_0000_1000_0000;

/** The child of this cell is linked to parents on the left. */
export const LEFT_MERGE_PARENT: LinkLine = 0b0_0001_0000_0000;

/** The child of this cell is linked to ancestors on the left. */
export const LEFT_MERGE_ANCESTOR: LinkLine = 0b0_0010_0000_0000;

/** The child of this cell is linked to parents on the right. */
export const RIGHT_MERGE_PARENT: LinkLine = 0b0_0100_0000_0000;

/** The child of this cell is linked to ancestors on the right. */
export const RIGHT_MERGE_ANCESTOR: LinkLine = 0b0_1000_0000_0000;

/**
 * The target node of this link line is the child of this column.
 *
 * This disambiguates between the node that is connected in this link line, and
 * other nodes that are also connected vertically.
 */
export const CHILD: LinkLine = 0b1_0000_0000_0000;

export const HORIZONTAL: LinkLine = HORIZ_PARENT | HORIZ_ANCESTOR;
export const VERTICAL: LinkLine = VERT_PARENT | VERT_ANCESTOR;
export const LEFT_FORK: LinkLine = LEFT_FORK_PARENT | LEFT_FORK_ANCESTOR;
export const RIGHT_FORK: LinkLine = RIGHT_FORK_PARENT | RIGHT_FORK_ANCESTOR;
export const LEFT_MERGE: LinkLine = LEFT_MERGE_PARENT | LEFT_MERGE_ANCESTOR;
export const RIGHT_MERGE: LinkLine = RIGHT_MERGE_PARENT | RIGHT_MERGE_ANCESTOR;
export const ANY_MERGE: LinkLine = LEFT_MERGE | RIGHT_MERGE;
export const ANY_FORK: LinkLine = LEFT_FORK | RIGHT_FORK;
export const ANY_FORK_OR_MERGE: LinkLine = ANY_MERGE | ANY_FORK;
