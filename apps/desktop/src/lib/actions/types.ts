import { Transform, Type } from 'class-transformer';
export type UpdatedBranch = {
	/** The name of the branch that was updated. */
	branchName: string;
	/** The list of commits resulting from the update. */
	newCommits: string[];
};

/** Represents the outcome of an action performed by a GitButler automation. */
export type Outcome = {
	updatedBranches: UpdatedBranch[];
};

export type ActionHandler = 'handleChangesSimple';
/** Represents a snapshot of an automatic action taken by a GitButler automation.  */
export class ButlerAction {
	/** UUID identifier of the action */
	id!: string;
	/** The time when the action was performed. */
	@Transform((obj) => new Date(obj.value))
	createdAt!: Date;
	/** A description of the change that was made and why it was made - i.e. the information that can be obtained from the caller. */
	externalSummary!: string;
	/** The prompt used that triggered this thingy stuff figgure it out yourself */
	externalPrompt!: string;
	/** The handler / implementation that performed the action. */
	handler!: ActionHandler;
	/** A GitBulter Oplog snapshot ID before the action was performed. */
	snapshotBefore!: string;
	/** A GitBulter Oplog snapshot ID after the action was performed. */
	snapshotAfter!: string;
	/** The outcome of the action, if it was successful. */
	response?: Outcome;
	/** An error message if the action failed. */
	error?: string;
}

export class ActionListing {
	total!: number;
	@Type(() => ButlerAction)
	actions!: ButlerAction[];
}
