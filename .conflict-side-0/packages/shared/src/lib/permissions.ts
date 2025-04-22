export enum ShareLevel {
	Public = 'public',
	Private = 'private',
	Unlisted = 'unlisted'
}

export type ApiPermissions = {
	can_read: boolean;
	can_write: boolean;
	share_level: ShareLevel;
};

export type Permissions = {
	canRead: boolean;
	canWrite: boolean;
	shareLevel: ShareLevel;
};

export function apiToPermissions(api: ApiPermissions): Permissions {
	return {
		canRead: api.can_read,
		canWrite: api.can_write,
		shareLevel: api.share_level
	};
}
