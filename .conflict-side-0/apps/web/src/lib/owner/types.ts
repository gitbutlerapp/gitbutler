/**
 * Types for owner data (users and organizations)
 */

import type { Organization } from '$lib/types';
import type { Organization as SharedOrganization } from '@gitbutler/shared/organizations/types';
import type { Project } from '@gitbutler/shared/organizations/types';
import type { User as SharedUser } from '@gitbutler/shared/users/types';

export type ProjectPermissions = {
	can_read: boolean;
	can_write: boolean;
	share_level: string;
};

export type OrganizationMember = {
	organization_slug: string;
	role: string;
	login: string;
	name: string;
	avatar_url: string;
};

// Extended types that include additional fields needed by the owner service
export type ExtendedUser = SharedUser & {
	readme?: string;
	organizations?: Organization[];
};

export type ExtendedOrganization = SharedOrganization & {
	avatarUrl?: string;
	projects?: Project[];
	members?: OrganizationMember[];
};

export type OwnerResponse =
	| {
			type: 'user';
			data: ExtendedUser;
	  }
	| {
			type: 'organization';
			data: ExtendedOrganization;
	  }
	| {
			type: 'not_found';
	  };

// Type for loading state
export type LoadableOwner = {
	status: 'loading' | 'found' | 'not-found' | 'error';
	slug: string;
	value?: OwnerResponse;
	error?: string;
};
