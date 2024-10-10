import { persisted, type Persisted } from '@gitbutler/shared/persisted';

const PR_DEFAULT_ACTION_KEY_NAME = 'projectDefaultPrAction';

export enum PRAction {
	Create = 'createPr',
	CreateDraft = 'createDraftPr'
}

export const prActions = Object.values(PRAction);

export const PRActionLabels = {
	[PRAction.Create]: 'Create PR',
	[PRAction.CreateDraft]: 'Create Draft PR'
} as const;

export function getPreferredPRAction(): Persisted<PRAction> {
	return persisted<PRAction>(PRAction.Create, PR_DEFAULT_ACTION_KEY_NAME);
}
