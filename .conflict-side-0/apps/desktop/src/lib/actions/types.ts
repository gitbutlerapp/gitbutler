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

export type ActionSource =
	| 'ButCli'
	| 'GitButler'
	| 'Unknown'
	| {
			Mcp: {
				name: string;
				version: string;
			} | null;
	  };
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
	/** The source of the action, if known. */
	source!: ActionSource;
}

export class ActionListing {
	total!: number;
	@Type(() => ButlerAction)
	actions!: ButlerAction[];
}

type RewordKind = {
	type: 'reword';
	subject: {
		stackId: string;
		branchName: string;
		commitId: string;
		newMessage: string;
	} | null;
};

export type WorkflowKind = RewordKind;

export function getDisplayNameForWorkflowKind(kind: WorkflowKind): string {
	switch (kind.type) {
		case 'reword':
			return 'Improved commit message';
	}
}

export type Trigger =
	| { readonly type: 'manual' }
	| { readonly type: 'snapshot'; readonly subject: string }
	| { readonly type: 'unknown' };

export type Status =
	| { readonly type: 'completed' }
	| { readonly type: 'failed'; readonly subject: string }
	| { readonly type: 'interupted'; readonly subject: string };

/** Represents a workflow that was executed by GitButler. */
export class Workflow {
	/** UUID identifier of the workflow */
	id!: string;
	/** The time when the workflow was captured. */
	@Transform((obj) => new Date(obj.value))
	createdAt!: Date;
	/** The name of the workflow that was performed */
	kind!: WorkflowKind;
	/** The trigger that initiated the workflow. */
	triggeredBy!: Trigger;
	/** The status of the workflow. */
	status!: Status;
	/** Input commits */
	inputCommits!: string[];
	/** Output commits */
	outputCommits!: string[];
	/** Optional summary of the workflow */
	summary?: string;
}

export class WorkflowList {
	total!: number;
	@Type(() => Workflow)
	workflows!: Workflow[];
}
