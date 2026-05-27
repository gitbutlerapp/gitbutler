import type { LinkLine } from "@gitbutler/but-sdk";

export const intersects = (x: LinkLine, y: LinkLine): boolean => (x & y) !== 0;

/**
 * This cell contains a horizontal line that connects to a parent.
 *
 * @public
 */
export const HORIZ_PARENT: LinkLine = 0b0_0000_0000_0001;

/**
 * This cell contains a horizontal line that connects to an ancestor.
 *
 * @public
 */
export const HORIZ_ANCESTOR: LinkLine = 0b0_0000_0000_0010;

/**
 * The descendent of this cell is connected to the parent.
 *
 * @public
 */
export const VERT_PARENT: LinkLine = 0b0_0000_0000_0100;

/**
 * The descendent of this cell is connected to an ancestor.
 *
 * @public
 */
export const VERT_ANCESTOR: LinkLine = 0b0_0000_0000_1000;

/**
 * The parent of this cell is linked in this link row and the child is to the
 * left.
 *
 * @public
 */
export const LEFT_FORK_PARENT: LinkLine = 0b0_0000_0001_0000;

/**
 * The ancestor of this cell is linked in this link row and the child is to the
 * left.
 *
 * @public
 */
export const LEFT_FORK_ANCESTOR: LinkLine = 0b0_0000_0010_0000;

/**
 * The parent of this cell is linked in this link row and the child is to the
 * right.
 *
 * @public
 */
export const RIGHT_FORK_PARENT: LinkLine = 0b0_0000_0100_0000;

/**
 * The ancestor of this cell is linked in this link row and the child is to the
 * right.
 *
 * @public
 */
export const RIGHT_FORK_ANCESTOR: LinkLine = 0b0_0000_1000_0000;

/**
 * The child of this cell is linked to parents on the left.
 *
 * @public
 */
export const LEFT_MERGE_PARENT: LinkLine = 0b0_0001_0000_0000;

/**
 * The child of this cell is linked to ancestors on the left.
 *
 * @public
 */
export const LEFT_MERGE_ANCESTOR: LinkLine = 0b0_0010_0000_0000;

/**
 * The child of this cell is linked to parents on the right.
 *
 * @public
 */
export const RIGHT_MERGE_PARENT: LinkLine = 0b0_0100_0000_0000;

/**
 * The child of this cell is linked to ancestors on the right.
 *
 * @public
 */
export const RIGHT_MERGE_ANCESTOR: LinkLine = 0b0_1000_0000_0000;

/**
 * The target node of this link line is the child of this column.
 *
 * This disambiguates between the node that is connected in this link line, and
 * other nodes that are also connected vertically.
 *
 * @public
 */
export const CHILD: LinkLine = 0b1_0000_0000_0000;

/** @public */
export const HORIZONTAL: LinkLine = HORIZ_PARENT | HORIZ_ANCESTOR;

/** @public */
export const VERTICAL: LinkLine = VERT_PARENT | VERT_ANCESTOR;

/** @public */
export const LEFT_FORK: LinkLine = LEFT_FORK_PARENT | LEFT_FORK_ANCESTOR;

/** @public */
export const RIGHT_FORK: LinkLine = RIGHT_FORK_PARENT | RIGHT_FORK_ANCESTOR;

/** @public */
export const LEFT_MERGE: LinkLine = LEFT_MERGE_PARENT | LEFT_MERGE_ANCESTOR;

/** @public */
export const RIGHT_MERGE: LinkLine = RIGHT_MERGE_PARENT | RIGHT_MERGE_ANCESTOR;

/** @public */
export const ANY_MERGE: LinkLine = LEFT_MERGE | RIGHT_MERGE;

/** @public */
export const ANY_FORK: LinkLine = LEFT_FORK | RIGHT_FORK;

/** @public */
export const ANY_FORK_OR_MERGE: LinkLine = ANY_MERGE | ANY_FORK;
